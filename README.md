# Wiggum Chess

Workspace Rust para desenvolvimento de uma engine de xadrez, execução de partidas entre engines, benchmarking de performance e iterações automatizadas de evolução da engine.

## Estrutura do workspace

- `chesslib` — núcleo da lógica de xadrez: tabuleiro, geração de movimentos, regras, PGN, relógio, integração com engines UCI e execução de partidas.
- `chess-engine` — engine UCI do projeto.
- `chess-runner` — CLI para rodar matches, SPRT, relatórios de versão, replay de PGN e extração de posições balanceadas.
- `perft-bench` — benchmark de perft comparando implementações.
- `chesslib-simple` — implementação alternativa/simplificada usada como referência de comparação.

## Requisitos

- Rust toolchain com Cargo
- Stockfish no `PATH` para fluxos que dependem de análise/benchmark externo
- `hyperfine` para `perft-bench/bench.sh`

## Build

```bash
cargo build --workspace
```

Builds específicos:

```bash
cargo build --release -p chess-engine
cargo build --release -p chess-runner
cargo build --release -p perft-bench
```

## Testes

Rodar todos os testes relevantes do workspace:

```bash
cargo test --workspace -- --skip gen_files::magics::name
```

Rodar apenas um crate:

```bash
cargo test -p chesslib
cargo test -p chess-engine
cargo test -p evolution-loop
```

Rodar um teste específico:

```bash
cargo test -p chesslib <nome_do_teste> -- --nocapture
```

## Rodando a engine

Executa a engine UCI local com profundidade fixa:

```bash
cargo run -p chess-engine -- --depth 5
```

## Rodando partidas entre engines

Exemplo simples de match:

```bash
cargo run -p chess-runner -- match \
  --engine1 ./target/release/chess-engine \
  --engine2 /caminho/para/outra/engine \
  --games 2
```

Exemplo de SPRT:

```bash
cargo run -p chess-runner -- sprt \
  --engine1 ./target/release/chess-engine \
  --engine2 /caminho/para/outra/engine
```

## Benchmarks

Perft rápido usando alias do Cargo:

```bash
cargo quick-bench
```

Benchmark cruzado entre implementações:

```bash
./perft-bench/bench.sh --position starting
```

Opções úteis:

```bash
./perft-bench/bench.sh --position kiwipete
./perft-bench/bench.sh --simple --position promotions
./perft-bench/bench.sh --python --position captures
```

## Benchmark de versões

Script para comparar uma versão da engine contra versão anterior e/ou Stockfish:

```bash
./scripts/benchmark-version.sh \
  --version v0.2 \
  --engine ./target/release/chess-engine \
  --prev-engine chess-engine/versions/v0.1/wiggum-engine
```

Os artefatos e relatórios de versão ficam em `chess-engine/versions/<tag>/`.

## Dados e artefatos

- `data/balanced-positions.fen` — posições balanceadas para benchmarking
- `data/lichess-2013-01.pgn` — base PGN usada em fluxos de extração/análise
- `chess-engine/versions/` — histórico de versões e relatórios

## Notas

- `chesslib` gera tabelas de movimento e chaves Zobrist em tempo de build via `build.rs`.
- O workspace usa `opt-level = 3` também em perfis de desenvolvimento e teste.
- Para validação do workspace, prefira:

```bash
cargo build --workspace
cargo test --workspace -- --skip gen_files::magics::name
```
