PI_ARCH := "aarch64-unknown-linux-gnu" 
PI_IP := "10.2.0.25"
PROJECT_NAME := "habit-tracker"

build:
	cross build --target {{PI_ARCH}} --release

test:
	cross test --target {{PI_ARCH}}

copy:
	scp target/{{PI_ARCH}}/release/{{PROJECT_NAME}} aiden@{{PI_IP}}:/home/aiden/{{PROJECT_NAME}}/habit-tracker

clippy:
	cross clippy --target {{PI_ARCH}} --no-deps

restart:
	ssh aiden@{{PI_IP}} "sudo systemctl restart {{PROJECT_NAME}}"

stop:
	ssh aiden@{{PI_IP}} "sudo systemctl stop {{PROJECT_NAME}}"

deploy: stop copy restart
