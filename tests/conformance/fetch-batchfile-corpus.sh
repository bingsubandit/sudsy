#!/usr/bin/env bash
# Fetches the pinned projects of the Batchfile evidence (docs/ai/design/batchfile.md) into
# tests/conformance/corpus/batchfile/<repo> (gitignored). The 32 projects were chosen before any result was seen;
# every `*.bat` and `*.cmd` file of a project, except vendored and third-party trees. `evidence_batchfile.py` caps a
# project at 40 files (first by path) so that no project outweighs the others.
export TARBALL_LENIENT=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*.bat *.cmd !*/vendor/* !*/node_modules/* !*/third_party/* !*/thirdparty/* !*/testdata/*'
# one call per project: a project with no such file at all is reported and skipped
for line in \
  'apache/tomcat e60606722bdd2decfafdff19b612c7a8c0e94d6b $P' \
  'apache/kafka 39872ea0c7d02eb92fc0a985c719b3f3958a4193 $P' \
  'apache/maven 41394fa11e28d2a8ad07cb4863e4d6add9c2788b $P' \
  'apache/cassandra 25cb50d1f7ced6249baf9a660f4cfbd6605fcf5f $P' \
  'apache/hadoop 907ef6c2858dc42d83cc228d13409830a7d7b163 $P' \
  'apache/spark 3003cd04d06ff53cfed3dba2b69260110596f9cd $P' \
  'elastic/elasticsearch c508ded59bd9ccb13235c156ad26e75c40c9f9d2 $P' \
  'nodejs/node 72bf3926bfa385adc9c3ebdfd6c1f4c9ee33cbf0 $P' \
  'python/cpython 3b3093726ffd07ebba5b906a1a55e453532e78b7 $P' \
  'php/php-src 5701fc611ecb3bd39a2323804b1e1811afb45f4f $P' \
  'ruby/ruby 2006d39eec99f6fd578a146a60029b80ea397ae8 $P' \
  'rust-lang/rust 42e0b13fb7310a12a827f6a28891c11d2e10923a $P' \
  'microsoft/terminal 43c7ec76a2eb64dac184358fbad25040c9f4a24f $P' \
  'dotnet/runtime 4fc48303381826ab92b8fb379d8e573ab397d71c $P' \
  'dotnet/aspnetcore d0cdff505ed9d46dc7f59888404b29d7a485ce4d $P' \
  'massgravel/Microsoft-Activation-Scripts 6cfd6f455ffb4983ce567231d800f08c44c39f00 $P' \
  'bmrf/tron 552aa363f17a00958125d81f4004c92a168ef3ca $P' \
  'cmderdev/cmder 5655c5eb90a075b8bcdd8617bd331bd3dd96c5c2 $P' \
  'vim/vim 6c201ec5d890c649ada74bd1c1975fdf65a54d7f $P' \
  'JetBrains/intellij-community cdcde444e0bae81434ed8ad35c2fb6543d5a26e8 $P' \
  'llvm/llvm-project dcf912c61e84b7403b792b6562954a7cbb08a631 $P' \
  'chocolatey/choco 2fa8bd4ff4e8af1420e3659f53a24a3019b2d0a0 $P' \
  'git-for-windows/git 01d4ffcc162b9772881c347654f206ba8d091f06 $P' \
  'tensorflow/tensorflow 7d13f04891977716c3d80e9a89a88d7b1e95af72 $P' \
  'opencv/opencv 2d579693d87aa81224bfda0c2f92b63d4745a68a $P' \
  'Kitware/CMake 801b483c23c60e6c29b6bfd6431eb1d288257a02 $P' \
  'openjdk/jdk fa4fd40faf916f87a0d74a13051291e92cee4d87 $P' \
  'pytorch/pytorch bcfd8341c82e9e72c81aebd58439272461bd7820 $P' \
  'electron/electron d98841c11ec9f890f3ba339b02f210f8012f1d49 $P' \
  'emscripten-core/emscripten 26fa03e2ad5aba9a2be222c83cc980824fa3c0b1 $P' \
  'protocolbuffers/protobuf e5bed28d750c106809c00ab179f56ed5826474b0 $P' \
  'wireshark/wireshark 97b0826c248400788b03936a8f05151fd2369727 $P' \
  'gradle/gradle 80e3397340ca2cdabb3a4f429ae3a7baa7f755e9 $P' \
  ; do
  fetch_tarballs batchfile <<<"${line//\$P/$P}" || echo "no batch file: ${line%% *}"
done
