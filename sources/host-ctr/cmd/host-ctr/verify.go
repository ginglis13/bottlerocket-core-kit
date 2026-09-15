package main

import (
	"context"
	"fmt"
	"io"

	"github.com/containerd/containerd"
	"github.com/containerd/containerd/content"
	"github.com/containerd/containerd/images"
	"github.com/containerd/containerd/reference"
	"github.com/containerd/errdefs"
	"github.com/containerd/log"
	"github.com/containerd/platforms"
	"github.com/opencontainers/go-digest"
	ocispec "github.com/opencontainers/image-spec/specs-go/v1"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

// parseExpectedDigest determines the digest an image must match before it may be used.
//
// The digest comes from the `--expected-digest` flag when given, and otherwise from a
// `@<digest>` suffix on the image source, if present. When both are given they must agree.
// An empty result means the image is not pinned and no verification is performed.
//
// The expected digest is that of the image's top-level manifest or index, which is what a
// registry reports for the image and what containerd stores as the image target.
func parseExpectedDigest(flagValue string, source string) (digest.Digest, error) {
	var fromFlag digest.Digest
	if flagValue != "" {
		parsed, err := digest.Parse(flagValue)
		if err != nil {
			return "", errors.Wrapf(err, "invalid expected digest %q", flagValue)
		}
		if parsed.Algorithm() != digest.SHA256 {
			return "", fmt.Errorf("unsupported expected digest algorithm %q: only sha256 is supported", parsed.Algorithm())
		}
		fromFlag = parsed
	}

	var fromRef digest.Digest
	// An unparseable source is reported by the pull itself; nothing to pin here.
	if spec, err := reference.Parse(source); err == nil {
		if refDigest := spec.Digest(); refDigest != "" {
			if err := refDigest.Validate(); err != nil {
				return "", errors.Wrapf(err, "invalid digest in image source %q", source)
			}
			fromRef = refDigest
		}
	}

	switch {
	case fromFlag != "" && fromRef != "" && fromFlag != fromRef:
		return "", fmt.Errorf("expected digest %s conflicts with digest %s in image source", fromFlag, fromRef)
	case fromFlag != "":
		return fromFlag, nil
	default:
		return fromRef, nil
	}
}

// verifyImageContent checks that an image stored in the content store is exactly the one
// identified by the expected digest.
//
// The image target must carry the expected digest, and every blob reachable from it for the
// given platform (the index, the platform's manifest, its config, and each layer) must be
// present and hash to the digest its descriptor claims. Only the platform's manifest is
// checked for a multi-platform index, since that is the only one containerd fetches.
func verifyImageContent(ctx context.Context, provider content.Provider, target ocispec.Descriptor, expected digest.Digest, platform platforms.MatchComparer) error {
	if target.Digest != expected {
		return fmt.Errorf("image digest mismatch: expected %s, got %s", expected, target.Digest)
	}
	if err := verifyBlob(ctx, provider, target); err != nil {
		return err
	}

	manifest := target
	if images.IsIndexType(target.MediaType) {
		children, err := images.Children(ctx, provider, target)
		if err != nil {
			return errors.Wrapf(err, "failed to read image index %s", target.Digest)
		}
		var best *ocispec.Descriptor
		for i := range children {
			child := &children[i]
			if child.Platform == nil || !platform.Match(*child.Platform) {
				continue
			}
			if best == nil || platform.Less(*child.Platform, *best.Platform) {
				best = child
			}
		}
		if best == nil {
			return fmt.Errorf("image index %s has no manifest for this platform", target.Digest)
		}
		if err := verifyBlob(ctx, provider, *best); err != nil {
			return err
		}
		manifest = *best
	}

	switch {
	case manifest.MediaType == images.MediaTypeDockerSchema1Manifest:
		return fmt.Errorf("cannot verify schema 1 image manifest %s", manifest.Digest)
	case !images.IsManifestType(manifest.MediaType):
		return fmt.Errorf("unexpected media type %q for image manifest %s", manifest.MediaType, manifest.Digest)
	}

	blobs, err := images.Children(ctx, provider, manifest)
	if err != nil {
		return errors.Wrapf(err, "failed to read image manifest %s", manifest.Digest)
	}
	for _, blob := range blobs {
		if err := verifyBlob(ctx, provider, blob); err != nil {
			return err
		}
	}
	return nil
}

// verifyBlob re-reads a blob from the content store and checks that its size and digest match
// its descriptor. The content is streamed, so large layers are not held in memory.
func verifyBlob(ctx context.Context, provider content.Provider, desc ocispec.Descriptor) error {
	if err := desc.Digest.Validate(); err != nil {
		return errors.Wrapf(err, "invalid digest for %s blob", desc.MediaType)
	}
	ra, err := provider.ReaderAt(ctx, desc)
	if err != nil {
		if errdefs.IsNotFound(err) {
			return fmt.Errorf("blob %s (%s) is missing from the content store", desc.Digest, desc.MediaType)
		}
		return errors.Wrapf(err, "failed to open blob %s", desc.Digest)
	}
	defer ra.Close()

	if ra.Size() != desc.Size {
		return fmt.Errorf("blob %s size mismatch: expected %d bytes, found %d", desc.Digest, desc.Size, ra.Size())
	}
	digester := desc.Digest.Algorithm().Digester()
	if _, err := io.Copy(digester.Hash(), content.NewReader(ra)); err != nil {
		return errors.Wrapf(err, "failed to read blob %s", desc.Digest)
	}
	if actual := digester.Digest(); actual != desc.Digest {
		return fmt.Errorf("blob %s content mismatch: stored content hashes to %s", desc.Digest, actual)
	}
	return nil
}

// verifyFetched verifies an image from the image store against the expected digest, if one was
// given. On failure the image record is removed so that a later run cannot pick it up as a
// cached image; running containers are unaffected since their snapshots keep the content alive.
func verifyFetched(ctx context.Context, client *containerd.Client, img containerd.Image, expected digest.Digest) error {
	if expected == "" {
		return nil
	}
	logger := log.G(ctx).WithFields(logrus.Fields{
		"img":             img.Name(),
		"expected-digest": expected,
	})
	err := verifyImageContent(ctx, client.ContentStore(), img.Target(), expected, platforms.Default())
	if err == nil {
		logger.Info("verified image content against expected digest")
		return nil
	}

	logger.WithError(err).WithField("target-digest", img.Target().Digest).Error("image failed digest verification, removing it from the image store")
	if derr := client.ImageService().Delete(ctx, img.Name(), images.SynchronousDelete()); derr != nil && !errdefs.IsNotFound(derr) {
		logger.WithError(derr).Error("failed to remove unverified image")
	}
	return errors.Wrap(err, "image digest verification failed")
}
