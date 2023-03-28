SHELL:=bash
#
# Build docker image
#
#

NAME=pg-event-server

BUILDID=$(shell date +"%Y%m%d%H%M")
COMMITID=$(shell git rev-parse --short HEAD)

ifdef CI_COMMIT_TAG
BUILDVERSION:=$(CI_COMMIT_TAG)
else
BUILDVERSION:=dev
endif

BUILDIMAGE:=$(NAME):$(BUILDVERSION)

ifdef REGISTRY_URL
REGISTRY_PREFIX=$(REGISTRY_URL)/
BUILD_ARGS += --build-arg REGISTRY_PREFIX=$(REGISTRY_PREFIX)
endif

ifndef REGISTRY_PREFIX
REGISTRY_TAG_PREFIX:=3liz/
else
REGISTRY_TAG_PREFIX:=$(REGISTRY_PREFIX)
endif

MANIFEST=factory.manifest

build: manifest
	cd .. && docker build $(BUILD_ARGS) -t $(BUILDIMAGE) -f .docker/Dockerfile .

manifest:
	{ \
	set -e; \
	echo name=$(NAME) > $(MANIFEST) && \
	echo version=$(BUILDVERSION) >> $(MANIFEST) && \
	echo release_tag=latest && \
	echo buildid=$(BUILDID)   >> $(MANIFEST) && \
	echo commitid=$(COMMITID) >> $(MANIFEST); }

deliver: tag push

tag:
	docker tag $(BUILDIMAGE) $(REGISTRY_TAG_PREFIX)$(NAME):$(BUILDVERSION)
ifdef CI_COMMIT_TAG
	docker tag $(BUILDIMAGE) $(REGISTRY_TAG_PREFIX)$(NAME):latest
endif

push:
	docker push $(REGISTRY_TAG_PREFIX)$(NAME):$(BUILDVERSION)
ifdef CI_COMMIT_TAG
	docker push $(REGISTRY_TAG_PREFIX)$(NAME):latest
endif

