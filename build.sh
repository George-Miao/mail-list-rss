command -v apt && \
 apt-get update  && \
 apt-get upgrade && \
 apt-get install curl gcc g++ make  && \
 curl -fsSL https://deb.nodesource.com/setup_current.x | bash - && \
 apt-get install -y nodejs
cd front
npm i && npm run build
cd ..
cargo build --release