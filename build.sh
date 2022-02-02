command -v apt-get update -y && \
 apt-get upgrade -y && \
 apt-get install -y curl gcc g++ make  && \
 curl -fsSL https://deb.nodesource.com/setup_current.x | bash - && \
 apt-get install -y nodejs
cd front
npm i && npm run build
cd ..
cargo build --release