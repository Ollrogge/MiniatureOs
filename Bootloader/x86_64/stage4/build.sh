#!/bin/bash

cargo +nightly build --release -Zbuild-std=core --target x86-stage4.json -Zbuild-std-features=compiler-builtins-mem
