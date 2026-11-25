#!/usr/bin/env bash
# Fetches the pinned projects of the Jupyter evidence (docs/ai/design/jupyter.md) into
# tests/conformance/corpus/jupyter/<repo> (gitignored). The 50 projects were chosen before any result was seen
# (well known repositories with notebooks, several with kernels other than Python); every `*.ipynb` file, except
# checkpoints and vendored trees. `scripts/evidence_jupyter.py` caps a project at 40 files (first by path).
export TARBALL_LENIENT=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*.ipynb !*/.ipynb_checkpoints/* !*/node_modules/* !*/vendor/* !*/third_party/*'
# one call per project: a project with no such file at all is reported and skipped
for line in \
  'tensorflow/docs 2677d0294365f3406175e4f0030438b1cbe74b15 $P' \
  'keras-team/keras-io f5891b3cd4df69f6bd5a3c26ea92310f3a973626 $P' \
  'huggingface/notebooks 12bc7b53de6715a8822ea8d43dc07029bc202357 $P' \
  'fastai/fastbook 971f0289c3d0bda81868d3c028a832c6ee380b58 $P' \
  'fastai/course22 230390584ba5b990e20487d68681a9ec5b258eff $P' \
  'jax-ml/jax 70d1868147ed19f3786fb741b66c18e9de3f1771 $P' \
  'ageron/handson-ml3 ef1c9f5ab11083584635b9b3a7846f49c58a6084 $P' \
  'jakevdp/PythonDataScienceHandbook d66231454ef753818dc9213c9b5942e067266966 $P' \
  'jupyterlab/jupyterlab 585894cf7b1a5a226d579bcd94c9e759df5ceb86 $P' \
  'jupyter/notebook 5bc2024fa0453ef28039749e8ea036e0dcbb734f $P' \
  'ipython/ipython 3804fa83aff26219cea20920ac9082bc98b9a9be $P' \
  'numpy/numpy-tutorials 2c2b79dcbf72a163aed5b2173cb9089a40fe0003 $P' \
  'pymc-devs/pymc-examples ab4b347e91b648c84d4dd763625e0bb54ac4cdd5 $P' \
  'probml/pyprobml d1ea4122dcf3c2a2c02eb4e05343203b26440827 $P' \
  'astropy/astropy-tutorials 30bd5ef52e59221f5983493ad7139c4374b16e66 $P' \
  'statsmodels/statsmodels b567774bc312595407a79086c933ba0afab4d498 $P' \
  'plotly/plotly.py 2b5fddb50775aa0140ccf641964fb2903b4b078b $P' \
  'holoviz/hvplot 3af807e52270337c01175679a7fea4e5a02ca519 $P' \
  'dask/dask-examples 03cfcca533fae6647f7df0b1f6b350938168d0a0 $P' \
  'openai/openai-cookbook dfd760b4de0bd175b51ad05b9b5260c36dfea09c $P' \
  'anthropics/anthropic-cookbook 8ad5de647d4f0f50c95d74aabc8aa20a3f70cdd3 $P' \
  'langchain-ai/langchain a2d98c255a705e577019bd7b747c5073747e25b5 $P' \
  'run-llama/llama_index c8115a04f9a33dee85133a7f5c9a84aa5da007af $P' \
  'microsoft/ML-For-Beginners 8e5c0dfffdb56181450363f9ccd880bc69737ae8 $P' \
  'JuliaLang/IJulia.jl 0210ca2b9ffaea9a69161ce4e997c37b20dc2d6b $P' \
  'google/evcxr e2a29f18b62ad5e6befaa27e157d8f8efc2b8156 $P' \
  'dotnet/interactive 098a01b9183103186805a186a7dee3a3a70b2997 $P' \
  'jupyter-xeus/xeus-cling 3788b129dbacda2c5653e787412bbfb0319d6c7b $P' \
  'scikit-learn/scikit-learn 7c426b6d7f5eacc57abfd58880cf6187bd09cbdc $P' \
  'scikit-image/scikit-image 2b5601b68184d66b241eb6c94286918452f0cf3c $P' \
  'networkx/networkx c220ac4fa9ee3f42ef927b8bf13fc933924ae763 $P' \
  'geopandas/geopandas d6ff79e9f91f60c463c72fe864a79752f9811d71 $P' \
  'pandas-dev/pandas 52552c59c1bd99db676b8eb103f591ee8cf1d93d $P' \
  'matplotlib/matplotlib ed25523818fb2fc1573225a5d13b6e32f4ff6070 $P' \
  'jupyter/nbformat cfb73afdd2a7f358953f2a34f747fd0d0e9d6288 $P' \
  'jupyter-widgets/ipywidgets 8ada3542abe85fdc52e2bfeeecc07f8db351cb9c $P' \
  'pytorch/tutorials 5277794075fd393cf24f139ac978f96bc06b5aa3 $P' \
  'mne-tools/mne-python c128077643f905ebf8b75a6f7664c8c20243e39c $P' \
  'Unidata/MetPy 5edfa0186fb0d01ef6d29319c6c2a496a888fb28 $P' \
  'pyro-ppl/pyro a31bc1784707268cd79df77ead4c7a47062f0193 $P' \
  'lmcinnes/umap 2157d26cefabb832e27cc66637a7b9d4e4b33f98 $P' \
  'mwaskom/seaborn c1cb287835852406691aed60ea0c62710f2c6b18 $P' \
  'rasbt/LLMs-from-scratch edf9488fce0cab4762b877864e9c2a9a3ef6b5a5 $P' \
  'karpathy/nn-zero-to-hero 73c3fcc741f0ec104ca850b1fb0df90e7e8d4cde $P' \
  'scipy/scipy 4709c22d6bac9e9e06c945b2290739a58f6f6c01 $P' \
  'IRkernel/IRkernel 124f2347f15cfadeabc6738868e05e6087f4c456 $P' \
  'almond-sh/almond a5f1005197431342c6d0e1451da42d287db0afed $P' \
  'gopherdata/gophernotes 900c58c5edf8e30c5a0aa4946b13aa914215d36c $P' \
  'takluyver/bash_kernel 8c58c7b4bd42fd290042f8922f657c1dd2ab5385 $P' \
  'nteract/nteract f2ed965f2bfac2e2b7cf9add2c654da1c821fd09 $P' \
  ; do
  fetch_tarballs jupyter <<<"${line//\$P/$P}" || echo "no notebook: ${line%% *}"
done
