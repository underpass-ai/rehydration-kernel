RELEASE_NAME ?= rehydration-kernel
NAMESPACE ?= underpass-runtime
VALUES_FILE ?= charts/rehydration-kernel/values.underpass-runtime.yaml
IMAGE_TAG ?= main
IMAGE_DIGEST ?=
HELM_TIMEOUT ?= 10m
WAIT_FOR_ROLLOUT ?= true
ATOMIC_DEPLOY ?= true

.PHONY: k8s-deploy k8s-dry-run k8s-status k8s-uninstall

k8s-deploy:
	RELEASE_NAME="$(RELEASE_NAME)" \
	NAMESPACE="$(NAMESPACE)" \
	VALUES_FILE="$(VALUES_FILE)" \
	IMAGE_TAG="$(IMAGE_TAG)" \
	IMAGE_DIGEST="$(IMAGE_DIGEST)" \
	HELM_TIMEOUT="$(HELM_TIMEOUT)" \
	WAIT_FOR_ROLLOUT="$(WAIT_FOR_ROLLOUT)" \
	ATOMIC_DEPLOY="$(ATOMIC_DEPLOY)" \
	DRY_RUN=false \
	bash scripts/ci/deploy-kubernetes.sh

k8s-dry-run:
	RELEASE_NAME="$(RELEASE_NAME)" \
	NAMESPACE="$(NAMESPACE)" \
	VALUES_FILE="$(VALUES_FILE)" \
	IMAGE_TAG="$(IMAGE_TAG)" \
	IMAGE_DIGEST="$(IMAGE_DIGEST)" \
	HELM_TIMEOUT="$(HELM_TIMEOUT)" \
	WAIT_FOR_ROLLOUT="$(WAIT_FOR_ROLLOUT)" \
	ATOMIC_DEPLOY="$(ATOMIC_DEPLOY)" \
	DRY_RUN=true \
	bash scripts/ci/deploy-kubernetes.sh

k8s-status:
	helm status "$(RELEASE_NAME)" --namespace "$(NAMESPACE)"

k8s-uninstall:
	helm uninstall "$(RELEASE_NAME)" --namespace "$(NAMESPACE)"
