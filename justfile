PI_ARCH := "aarch64-unknown-linux-gnu" 
PI_IP := "10.2.0.25"
PROJECT_NAME := "habit-tracker"

build:
	cross build --target {{PI_ARCH}}

test:
	cross test --target {{PI_ARCH}}

copy:
	scp target/{{PI_ARCH}}/debug/{{PROJECT_NAME}} aiden@{{PI_IP}}:/home/aiden/{{PROJECT_NAME}}

clippy:
	cross clippy --target {{PI_ARCH}} --no-deps
