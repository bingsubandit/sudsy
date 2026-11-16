#!/usr/bin/env bash
# Fetches the pinned PHP corpus into tests/conformance/corpus/php/<repo> (gitignored): Laravel,
# Symfony (with its test fixtures), WordPress and Composer (`.php` files).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs php <<'EOT'
laravel/framework b65b5e31522b8a3fcf2bcc7497fa6fb0e8f57ca1 *.php
symfony/symfony a9bd44fa9c1aec723ae497a41ead077c1986b4f4 *.php
WordPress/WordPress 6558c0972ee1826c016d6ab3173f36ebaf14de89 *.php
composer/composer 5147ea1a01c124f461090a72bee6ea6de02a67a6 *.php
EOT
