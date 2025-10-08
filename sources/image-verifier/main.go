package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
)

func main() {
	var digest = flag.String("digest", "", "image digest to verify")
	var name = flag.String("name", "", "image name to verify")
	var _ = flag.String("stdin-media-type", "", "image media type")
	flag.Parse()

	if *digest == "" || *name == "" {
		fmt.Fprintf(os.Stdout, "Usage: %s -digest <digest> -name <name>\n", os.Args[0])
		os.Exit(1)
	}

	fmt.Printf("verifying image: %s\n", *name)

	// Override the default notation paths to what we've packaged.
	os.Setenv("NOTATION_CONFIG", "/etc/notation")
	os.Setenv("NOTATION_CACHE", "/var/cache/notation")
	os.Setenv("NOTATION_LIBEXEC", "/usr/bin/notation-plugins")

	// The notation binary looks in $HOME/.docker/config.json for credentials.
	os.Setenv("HOME", "/root")

	cmd := exec.Command("notation", "verify", *name)
	output, err := cmd.CombinedOutput()

	if err != nil {
		fmt.Fprintln(os.Stdout, "Image verification failed")
		fmt.Fprintf(os.Stdout, "stderr: %s\n", string(output))
		os.Exit(1)
	}

	fmt.Fprintln(os.Stdout, "image verification successful")
	os.Exit(0)
}
