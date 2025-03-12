CC = x86_64-elf-gcc
CFLAGS = -nostdlib -fno-pic -static -Wall -Wextra -O2
LDFLAGS = -static

all: test

test: test.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o test test.c

clean:
	rm -f test
