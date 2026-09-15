package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/containerd/containerd/content"
	"github.com/containerd/containerd/content/local"
	"github.com/containerd/containerd/images"
	"github.com/containerd/platforms"
	"github.com/opencontainers/go-digest"
	"github.com/opencontainers/image-spec/specs-go"
	ocispec "github.com/opencontainers/image-spec/specs-go/v1"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// verifyStore is a local content store populated with synthetic image blobs for tests.
type verifyStore struct {
	t    *testing.T
	root string
	cs   content.Store
}

func newVerifyStore(t *testing.T) *verifyStore {
	root := t.TempDir()
	cs, err := local.NewStore(root)
	require.NoError(t, err)
	return &verifyStore{t: t, root: root, cs: cs}
}

// describe returns the descriptor for data without storing it.
func describe(mediaType string, data []byte) ocispec.Descriptor {
	return ocispec.Descriptor{
		MediaType: mediaType,
		Digest:    digest.FromBytes(data),
		Size:      int64(len(data)),
	}
}

// put stores a blob and returns its descriptor.
func (s *verifyStore) put(mediaType string, data []byte) ocispec.Descriptor {
	desc := describe(mediaType, data)
	err := content.WriteBlob(context.Background(), s.cs, "test-"+desc.Digest.Encoded(), bytes.NewReader(data), desc)
	require.NoError(s.t, err)
	return desc
}

// blobPath is where the local store keeps a blob on disk.
func (s *verifyStore) blobPath(d digest.Digest) string {
	return filepath.Join(s.root, "blobs", d.Algorithm().String(), d.Encoded())
}

// overwrite replaces a stored blob's bytes on disk without updating its descriptor.
func (s *verifyStore) overwrite(d digest.Digest, data []byte) {
	path := s.blobPath(d)
	require.NoError(s.t, os.Chmod(path, 0o644))
	require.NoError(s.t, os.WriteFile(path, data, 0o644))
}

func mustJSON(t *testing.T, v interface{}) []byte {
	data, err := json.Marshal(v)
	require.NoError(t, err)
	return data
}

func randomBytes(t *testing.T, n int) []byte {
	data := make([]byte, n)
	_, err := rand.Read(data)
	require.NoError(t, err)
	return data
}

// image is a stored single-platform image.
type image struct {
	manifest ocispec.Descriptor
	config   ocispec.Descriptor
	layers   []ocispec.Descriptor
}

// buildImage stores a config, `layerCount` layers, and a manifest for the platform, and returns
// their descriptors. The manifest descriptor carries the platform, as it would inside an index.
func (s *verifyStore) buildImage(platform ocispec.Platform, layerCount int) image {
	config := s.put(ocispec.MediaTypeImageConfig, mustJSON(s.t, ocispec.Image{Platform: platform}))
	var layers []ocispec.Descriptor
	for i := 0; i < layerCount; i++ {
		layers = append(layers, s.put(ocispec.MediaTypeImageLayerGzip, randomBytes(s.t, 4096+i)))
	}
	manifest := s.put(ocispec.MediaTypeImageManifest, mustJSON(s.t, ocispec.Manifest{
		Versioned: specs.Versioned{SchemaVersion: 2},
		MediaType: ocispec.MediaTypeImageManifest,
		Config:    config,
		Layers:    layers,
	}))
	manifest.Platform = &platform
	return image{manifest: manifest, config: config, layers: layers}
}

// buildIndex stores an index over the given manifests.
func (s *verifyStore) buildIndex(manifests ...ocispec.Descriptor) ocispec.Descriptor {
	return s.put(ocispec.MediaTypeImageIndex, mustJSON(s.t, ocispec.Index{
		Versioned: specs.Versioned{SchemaVersion: 2},
		MediaType: ocispec.MediaTypeImageIndex,
		Manifests: manifests,
	}))
}

func hostPlatform() ocispec.Platform {
	return platforms.DefaultSpec()
}

// otherPlatform is a Linux platform that is never the host's.
func otherPlatform() ocispec.Platform {
	arch := "s390x"
	if runtime.GOARCH == arch {
		arch = "ppc64le"
	}
	return ocispec.Platform{OS: "linux", Architecture: arch}
}

func TestVerifyImageContentSingleManifest(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 2)

	err := verifyImageContent(context.Background(), s.cs, img.manifest, img.manifest.Digest, platforms.Default())
	assert.NoError(t, err)
}

func TestVerifyImageContentIndex(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 3)

	// Descriptors for content containerd would not have fetched for this host: another
	// platform's manifest and an attestation manifest. Neither is stored.
	other := describe(ocispec.MediaTypeImageManifest, []byte("other-platform-manifest"))
	otherPlat := otherPlatform()
	other.Platform = &otherPlat
	attestation := describe(ocispec.MediaTypeImageManifest, []byte("attestation-manifest"))
	attestation.Platform = &ocispec.Platform{OS: "unknown", Architecture: "unknown"}
	attestation.Annotations = map[string]string{"vnd.docker.reference.type": "attestation-manifest"}

	index := s.buildIndex(other, img.manifest, attestation)

	err := verifyImageContent(context.Background(), s.cs, index, index.Digest, platforms.Default())
	assert.NoError(t, err)
}

func TestVerifyImageContentWrongExpectedDigest(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 1)
	wrong := digest.FromString("something else")

	err := verifyImageContent(context.Background(), s.cs, img.manifest, wrong, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "image digest mismatch")
	assert.Contains(t, err.Error(), wrong.String())
	assert.Contains(t, err.Error(), img.manifest.Digest.String())
}

func TestVerifyImageContentIndexWithoutHostPlatform(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(otherPlatform(), 1)
	index := s.buildIndex(img.manifest)

	err := verifyImageContent(context.Background(), s.cs, index, index.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "no manifest for this platform")
}

func TestVerifyImageContentMissingLayer(t *testing.T) {
	s := newVerifyStore(t)
	config := s.put(ocispec.MediaTypeImageConfig, mustJSON(t, ocispec.Image{Platform: hostPlatform()}))
	present := s.put(ocispec.MediaTypeImageLayerGzip, randomBytes(t, 1024))
	missing := describe(ocispec.MediaTypeImageLayerGzip, randomBytes(t, 1024))
	manifest := s.put(ocispec.MediaTypeImageManifest, mustJSON(t, ocispec.Manifest{
		Versioned: specs.Versioned{SchemaVersion: 2},
		MediaType: ocispec.MediaTypeImageManifest,
		Config:    config,
		Layers:    []ocispec.Descriptor{present, missing},
	}))

	err := verifyImageContent(context.Background(), s.cs, manifest, manifest.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "missing from the content store")
	assert.Contains(t, err.Error(), missing.Digest.String())
}

func TestVerifyImageContentTamperedLayer(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 2)

	// Same size, different bytes: only re-hashing the stored content can catch this.
	tampered := randomBytes(t, int(img.layers[1].Size))
	s.overwrite(img.layers[1].Digest, tampered)

	err := verifyImageContent(context.Background(), s.cs, img.manifest, img.manifest.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "content mismatch")
	assert.Contains(t, err.Error(), img.layers[1].Digest.String())
	assert.Contains(t, err.Error(), digest.FromBytes(tampered).String())
}

func TestVerifyImageContentTamperedConfig(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 1)
	s.overwrite(img.config.Digest, []byte(`{"os":"linux","architecture":"evil"}`))

	err := verifyImageContent(context.Background(), s.cs, img.manifest, img.manifest.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), img.config.Digest.String())
}

func TestVerifyImageContentTruncatedLayer(t *testing.T) {
	s := newVerifyStore(t)
	img := s.buildImage(hostPlatform(), 1)
	s.overwrite(img.layers[0].Digest, []byte("short"))

	err := verifyImageContent(context.Background(), s.cs, img.manifest, img.manifest.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "size mismatch")
}

func TestVerifyImageContentRejectsSchema1(t *testing.T) {
	s := newVerifyStore(t)
	manifest := s.put(images.MediaTypeDockerSchema1Manifest, []byte(`{"schemaVersion":1}`))

	err := verifyImageContent(context.Background(), s.cs, manifest, manifest.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "schema 1")
}

func TestVerifyImageContentRejectsNonManifestTarget(t *testing.T) {
	s := newVerifyStore(t)
	config := s.put(ocispec.MediaTypeImageConfig, mustJSON(t, ocispec.Image{Platform: hostPlatform()}))

	err := verifyImageContent(context.Background(), s.cs, config, config.Digest, platforms.Default())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "unexpected media type")
}

func TestVerifyImageContentDockerMediaTypes(t *testing.T) {
	s := newVerifyStore(t)
	config := s.put(images.MediaTypeDockerSchema2Config, mustJSON(t, ocispec.Image{Platform: hostPlatform()}))
	layer := s.put(images.MediaTypeDockerSchema2LayerGzip, randomBytes(t, 2048))
	manifest := s.put(images.MediaTypeDockerSchema2Manifest, mustJSON(t, ocispec.Manifest{
		Versioned: specs.Versioned{SchemaVersion: 2},
		MediaType: images.MediaTypeDockerSchema2Manifest,
		Config:    config,
		Layers:    []ocispec.Descriptor{layer},
	}))
	host := hostPlatform()
	manifest.Platform = &host
	list := s.put(images.MediaTypeDockerSchema2ManifestList, mustJSON(t, ocispec.Index{
		Versioned: specs.Versioned{SchemaVersion: 2},
		MediaType: images.MediaTypeDockerSchema2ManifestList,
		Manifests: []ocispec.Descriptor{manifest},
	}))

	err := verifyImageContent(context.Background(), s.cs, list, list.Digest, platforms.Default())
	assert.NoError(t, err)
}
