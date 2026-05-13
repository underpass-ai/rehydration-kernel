FROM docker.io/pytorch/pytorch:2.9.1-cuda13.0-cudnn9-devel

ENV DEBIAN_FRONTEND=noninteractive \
    HF_HOME=/cache/huggingface \
    TRANSFORMERS_CACHE=/cache/huggingface/transformers \
    PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True \
    MAX_JOBS=8 \
    TORCH_CUDA_ARCH_LIST=8.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN python -m pip install --upgrade pip setuptools wheel packaging ninja \
    && python -m pip install \
      "transformers==4.56.2" \
      "datasets>=3.6.0" \
      "peft>=0.15.0" \
      "accelerate>=1.7.0" \
      "trl>=0.18.0,<1.0" \
      sentencepiece \
      einops

RUN python -m pip install --no-build-isolation --no-deps \
      "flash-attn==2.7.3" \
      "causal-conv1d==1.5.0.post8" \
      "mamba-ssm==2.2.4" \
      "flash-linear-attention==0.5.0" \
      "fla-core==0.5.0"

RUN python - <<'PY'
import flash_attn
import mamba_ssm
print("nemotron operator dependencies ok")
PY

WORKDIR /workspace/repo
