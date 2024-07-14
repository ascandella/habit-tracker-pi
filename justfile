PI_ARCH := "aarch64-unknown-linux-gnu"
PI_IP := "10.2.0.25"
PROJECT_NAME := "workout-tracker-pi"

build:
	cargo zigbuild --release --target {{PI_ARCH}}

copy: build
	scp target/{{PI_ARCH}}/release/{{PROJECT_NAME}} aiden@{{PI_IP}}:/home/aiden/{{PROJECT_NAME}}
