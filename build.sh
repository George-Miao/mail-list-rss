command -v apt && \
 apt update  && \
 apt upgrade && \
 apt install curl && \
 curl -fsSL https://deb.nodesource.com/setup_current.x | bash -
cd front
npm run build
cd ..
cargo build --release