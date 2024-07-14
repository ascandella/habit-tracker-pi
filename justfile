PI_ARCH := "armv7-unknown-linux-gnueabihf"
PI_IP := "10.2.0.25"
PROJECT_NAME := "workout-tracker-pi"

build:
	cargo zigbuild --release --target {{PI_ARCH}}

copy: build
	scp target/{{PI_ARCH}}/release/{{PROJECT_NAME}} aiden@{{PI_IP}}:/home/aiden/{{PROJECT_NAME}}
