PI_ARCH := "aarch64-unknown-linux-gnu" 
PI_IP := "10.2.0.25"
PI_USER := "aiden"
PROJECT_NAME := "habit-tracker"

build:
	cross build --target {{PI_ARCH}} --release --color always

t:
	cross test --target {{PI_ARCH}} --color always

check:
	cross check --target {{PI_ARCH}} --color always

test:
	just t && just clippy

copy:
	scp target/{{PI_ARCH}}/release/{{PROJECT_NAME}} {{ PI_USER }}@{{PI_IP}}:/home/{{ PI_USER }}/{{PROJECT_NAME}}/habit-tracker

clippy:
	cross clippy --target {{PI_ARCH}} --no-deps

restart:
	ssh {{ PI_USER }}@{{PI_IP}} "sudo systemctl restart {{PROJECT_NAME}}"

stop:
	ssh {{ PI_USER }}@{{PI_IP}} "sudo systemctl stop {{PROJECT_NAME}}"

deploy: stop copy restart
