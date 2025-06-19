

PACKAGE_VERSION=$(shell cd ../pg-event-server && cargo metadata --format-version=1 --no-deps | jq -r '.packages[0].version')

IMAGE_NAME=$(REGISTRY_PREFIX)pg-event-server

ifndef BUILD_RELEASE
IMAGE_TAG=$(IMAGE_NAME):dev
else
IMAGE_TAG=$(IMAGE_NAME):$(PACKAGE_VERSION)
endif

REGISTRY_URL ?= 3liz
REGISTRY_PREFIX=$(REGISTRY_URL)/


build: 
	@echo "building $(IMAGE_TAG)"
	docker build --rm --label org.opencontainers.image.version=$(PACKAGE_VERSION) \
		-t $(IMAGE_TAG) -f Dockerfile ..
