.PHONY: build test check docker deploy status logs clean

build:
	cargo build --release

check:
	cargo check --workspace --all-targets

test:
	cargo test --workspace

docker:
	docker build -t titan-system:local .

deploy:
	./start.sh

status:
	kubectl -n titan get pods,svc

logs:
	kubectl -n titan logs deployment/titan-control-plane

clean:
	cargo clean
