#!/usr/bin/env bash
# Fetches the pinned Java corpus into tests/conformance/corpus/java/<repo> (ignored by Git): Guava,
# Gson and Error Prone (formatted with google-java-format), Commons Lang (not), and
# google-java-format itself with its own test inputs (testdata/*.input, *.output).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs java <<'EOT'
google/guava fce80fd984c8227262a63972ccc68ae6f2ad0519 *.java
google/gson e67037d17f94415179066c8301fee4a610d9515e *.java
google/error-prone f72c29da0eb4b0028294b3e93a9ab1bebb60612a */core/src/main/*.java */check_api/*.java
apache/commons-lang e2a0789bc936783fa2a75263412e3359d4c0f97b *.java
google/google-java-format 90e7671580810542a8908e0071e114d1215ef8a7 *.java */testdata/*
EOT
