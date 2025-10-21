#cargo build && zellij plugin --skip-plugin-cache -- file:/home/aram/code/picker/target/wasm32-wasip1/debug/picker.wasm
cargo build --release && cd /home/aram/code/zellij && /home/aram/backup/dev-zellij plugin --skip-plugin-cache -- file:/home/aram/code/picker/target/wasm32-wasip1/release/picker.wasm && cd /home/aram/code/picker
