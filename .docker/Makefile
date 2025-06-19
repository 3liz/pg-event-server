
ifdef CI_COMMIT_TAG
ifeq ($(shell echo $(CI_COMMIT_TAG) | head -c 8), release-)
	BUILD_RELEASE=$(shell date -u +%Y-%m-%dT%H-%M-%SZ)
endif
endif

NAME=pg-event-server

BUILDID=$(shell date +"%Y%m%d%H%M")
COMMITID=$(shell git rev-parse --short HEAD)

ifdef REGISTRY_URL
REGISTRY_PREFIX=$(REGISTRY_URL)/
BUILD_ARGS += --build-arg REGISTRY_PREFIX=$(REGISTRY_PREFIX)
else
REGISTRY_PREFIX=3liz/
endif

PACKAGE_VERSION=$(shell cd ../pg-event-server && cargo metadata --format-version=1 --no-deps | jq -r '.packages[0].version')

IMAGE_NAME=$(REGISTRY_PREFIX)$(NAME)

ifndef BUILD_RELEASE
IMAGE_TAG=$(IMAGE_NAME):dev
else
IMAGE_TAG=$(IMAGE_NAME):$(PACKAGE_VERSION)
endif

build: manifest
	@echo "building $(IMAGE_TAG)"
	docker build --rm --label org.opencontainers.image.version=$(PACKAGE_VERSION) \
		$(BUILD_ARGS) -t $(IMAGE_TAG) -f Dockerfile ..

MANIFEST=factory.manifest

manifest:
	{ \
	set -e; \
	echo name=$(NAME) > $(MANIFEST) && \
	echo version=$(PACKAGE_VERSION) >> $(MANIFEST) && \
	echo release_tag=latest && \
	echo buildid=$(BUILDID)   >> $(MANIFEST) && \
	echo commitid=$(COMMITID) >> $(MANIFEST); }

deliver: push

push:
	docker push $(IMAGE_TAG)
ifdef BUILD_RELEASE
	docker tag $(IMAGE_NAME):latest
	docker push $(IMAGE_NAME):latest
endif

