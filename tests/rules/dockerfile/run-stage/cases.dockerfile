FROM debian:12
RUN a
RUN b
^^^^^ merge-consecutive-run
# a comment between does not matter
RUN c
^^^^^ merge-consecutive-run
ENV A=1
RUN d
RUN e && f
RUN g
RUN h $(i)
RUN j
RUN --mount=type=cache,target=/x k
RUN --mount=type=cache,target=/x l
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ merge-consecutive-run
RUN m
RUN ["n", "o"]
^^^^^^^^^^^^^^ merge-consecutive-run
RUN ["p"]
^^^^^^^^^ merge-consecutive-run

RUN a | b
^^^^^^^^^ pipefail-shell
RUN a || b
RUN ["a", "b"]
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN a | b
FROM debian:12
RUN a | b
^^^^^^^^^ pipefail-shell
RUN apt-get update
^^^^^^^^^^^^^^^^^^ delete-apt-lists
RUN apt-get update && apt-get install -y a
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ delete-apt-lists
RUN apt update
^^^^^^^^^^^^^^ delete-apt-lists
RUN apt-get update && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y a && rm -fr "/var/lib/apt/lists"
RUN --mount=type=cache,target=/var/lib/apt/lists apt-get update
RUN apt-get install -y a
