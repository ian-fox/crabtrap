# Use rust so that it will work with the later environments
FROM rust:1 as base

WORKDIR /crabtrap_test
ENV LD_LIBRARY_PATH=/usr/local/lib
COPY sample_program/printf_wrapper.c \
    sample_program/dynamic.c \
    sample_program/static.c \
    ./
RUN gcc -c -o libprintf_wrapper.o printf_wrapper.c \
 && ar rcs libprintf_wrapper.a libprintf_wrapper.o \
 && gcc -shared -fPIC -o /usr/local/lib/libprintf_wrapper.so printf_wrapper.c \
 && gcc -o dynamic dynamic.c -ldl \
 && gcc -o static static.c -lprintf_wrapper \
 && gcc -static-pie -o all-in-one static.c -L. -l:libprintf_wrapper.a

FROM rust:1
 
COPY --from=base /usr/local/lib/libprintf_wrapper.so /usr/local/lib/
COPY --from=base /crabtrap_test/static \
    /crabtrap_test/dynamic \
    /crabtrap_test/all-in-one \
    /usr/local/bin/

WORKDIR /crabtrap
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY tests tests
