FROM rust:latest

# Installer quelques outils utiles
RUN apt-get update && apt-get install -y \
    bash \
    git \
    vim \
    nano \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Répertoire de travail
WORKDIR /workspace

# Lancer bash par défaut
CMD ["/bin/bash"]