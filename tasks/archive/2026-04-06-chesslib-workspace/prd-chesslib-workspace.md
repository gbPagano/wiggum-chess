# PRD: chesslib Workspace — Game, UCI e Engine Orchestration

## Introduction

Reestruturar o projeto atual em um Cargo workspace com duas crates: `chesslib` (biblioteca) e uma crate binária para orquestração de partidas. A `chesslib` será expandida para incluir detecção completa de fim de jogo (regras FIDE), uma struct `Game` com histórico de movimentos e controle de relógio, parsing de movimentos UCI, e uma abstração para plugar engines (tanto in-process via trait Rust quanto via subprocess UCI). O objetivo final é permitir que duas engines joguem uma partida completa entre si.

## Goals

- Migrar o projeto para um Cargo workspace com `chesslib` (lib) e `chess-runner` (bin)
- Implementar detecção de todas as condições de fim de jogo FIDE
- Criar uma struct `Game` que encapsule o estado completo de uma partida (histórico, relógio, resultado)
- Implementar parsing e serialização de moves no formato UCI (e.g. `e2e4`, `e7e8q`)
- Implementar subset do protocolo UCI suficiente para orquestrar partidas entre engines externas
- Abstrair engines via trait Rust para permitir engines in-process e subprocess UCI
- Permitir que o binário `chess-runner` execute uma partida entre duas engines quaisquer

## User Stories

### US-001: Converter projeto para Cargo workspace
**Description:** Como desenvolvedor, quero que o projeto seja um workspace com `chesslib` como subcrate, para que eu possa adicionar crates binárias separadas.

**Acceptance Criteria:**
- [ ] `Cargo.toml` raiz define um workspace com members `chesslib` e `chess-runner`
- [ ] `chesslib/Cargo.toml` contém a lib com todo o código existente (incluindo build.rs e gen_files)
- [ ] `chess-runner/Cargo.toml` define uma crate binária que depende de `chesslib`
- [ ] `cargo build --workspace` compila sem erros
- [ ] `cargo test --workspace` passa todos os testes existentes
- [ ] Benchmarks existentes continuam funcionando (`cargo bench --bench perft -p chesslib`)

### US-002: Detecção de checkmate e stalemate
**Description:** Como usuário da lib, quero saber se uma posição é checkmate ou stalemate, para determinar o fim de jogo.

**Acceptance Criteria:**
- [ ] Função/método que recebe um `Board` e retorna se é checkmate (sem movimentos legais + em xeque)
- [ ] Função/método que retorna se é stalemate (sem movimentos legais + não em xeque)
- [ ] Testes com posições conhecidas de checkmate (Scholar's mate, back rank mate)
- [ ] Testes com posições conhecidas de stalemate
- [ ] Testes de perft existentes continuam passando

### US-003: Detecção de draw por material insuficiente
**Description:** Como usuário da lib, quero detectar empate por material insuficiente (K vs K, K+B vs K, K+N vs K, K+B vs K+B same color).

**Acceptance Criteria:**
- [ ] Função que analisa o material restante e retorna se é insuficiente
- [ ] Cobre os casos: K vs K, K+N vs K, K+B vs K, K+B vs K+B (bispos na mesma cor)
- [ ] Testes para cada caso de material insuficiente
- [ ] Testes para posições com material suficiente (não retorna draw)

### US-004: Detecção de threefold/fivefold repetition
**Description:** Como usuário da lib, quero detectar repetição de posição para aplicar regras de empate FIDE.

**Acceptance Criteria:**
- [ ] `Game` mantém um histórico de posições (hash ou FEN) para comparação
- [ ] Método que retorna se a posição atual ocorreu 3 vezes (threefold — draw claimable)
- [ ] Método que retorna se a posição atual ocorreu 5 vezes (fivefold — draw automático)
- [ ] Hash de posição usa Zobrist hashing (chaves aleatórias por peça/square/castling/en passant/side to move)
- [ ] Zobrist hashing deve incluir uma chave para o turno (Side to Move) para garantir que posições idênticas com turnos diferentes tenham hashes diferentes
- [ ] Zobrist keys geradas como constantes (build.rs ou const)
- [ ] Hash é atualizado incrementalmente a cada movimento
- [ ] Testes com sequências de movimentos que produzem repetição

### US-005: Detecção de 50-move rule e 75-move rule
**Description:** Como usuário da lib, quero detectar empate pelas regras de 50 e 75 movimentos sem captura ou avanço de peão.

**Acceptance Criteria:**
- [ ] `Game` mantém halfmove clock (resetado em capturas e avanços de peão)
- [ ] Método que retorna se halfmove clock >= 100 (50-move rule — draw claimable)
- [ ] Método que retorna se halfmove clock >= 150 (75-move rule — draw automático)
- [ ] halfmove clock é inicializado corretamente a partir do FEN
- [ ] Testes com sequências que atingem 50 e 75 movimentos

### US-006: Struct Game com histórico de movimentos
**Description:** Como usuário da lib, quero uma struct `Game` que represente uma partida completa com histórico, para acompanhar o andamento do jogo.

**Acceptance Criteria:**
- [ ] `Game` contém: `Board` atual, lista de movimentos jogados (`Vec<ChessMove>`), resultado da partida
- [ ] Método `make_move()` que valida legalidade, aplica o movimento, atualiza histórico e detecta fim de jogo
- [ ] Método que retorna o `Board` atual
- [ ] Método que retorna o histórico de movimentos
- [ ] Enum `GameResult` com variantes: `Ongoing`, `WhiteWins`, `BlackWins`, `Draw(DrawReason)`
- [ ] `DrawReason` com variantes: `Stalemate`, `InsufficientMaterial`, `ThreefoldRepetition`, `FivefoldRepetition`, `FiftyMoveRule`, `SeventyFiveMoveRule`
- [ ] `Game::new()` a partir de um FEN ou posição inicial
- [ ] Testes para uma partida completa (abertura até mate)

### US-007: Controle de relógio no Game
**Description:** Como usuário da lib, quero que `Game` suporte controle de tempo com incremento, para compatibilidade com partidas UCI.

**Acceptance Criteria:**
- [ ] Struct `Clock` com tempo restante por jogador e incremento
- [ ] `Clock` é atualizado a cada movimento (subtrai tempo gasto, adiciona incremento)
- [ ] Suporte a configurações: tempo total + incremento, moves to go (para time controls clássicos)
- [ ] Detecção de flag (tempo esgotado) como condição de fim de jogo
- [ ] `Game` pode ser criado com ou sem relógio (relógio opcional)
- [ ] Testes para decrementação de tempo e detecção de flag

### US-008: Parsing e serialização de moves UCI
**Description:** Como usuário da lib, quero converter entre `ChessMove` e string UCI (e.g. `e2e4`, `e7e8q`), para comunicar com engines.

**Acceptance Criteria:**
- [ ] `ChessMove::from_uci(s: &str, board: &Board) -> Result<ChessMove>` — precisa do board para resolver ambiguidade de promoção e validar legalidade
- [ ] `ChessMove::to_uci() -> String` — serializa para formato UCI
- [ ] Formato: `{source}{dest}` (4 chars) ou `{source}{dest}{promotion}` (5 chars, promotion em lowercase: q/r/b/n)
- [ ] Testes para moves normais, capturas, castling (e1g1, e1c1), en passant, e promoções

### US-009: Trait Engine para abstração de engines
**Description:** Como desenvolvedor, quero uma trait `Engine` que abstraia a comunicação com engines, para plugar engines in-process ou UCI.

**Acceptance Criteria:**
- [ ] Trait `Engine` com métodos: `name() -> String`, `new_game()`, `set_position(game: &Game)`, `go(time_control: &TimeControl) -> ChessMove`, `quit()`
- [ ] `TimeControl` struct/enum que encapsula wtime, btime, winc, binc, movestogo
- [ ] Trait é async (`async fn`) usando `tokio` — métodos retornam futures
- [ ] Documentação da trait com exemplo de implementação mínima

### US-010: Engine UCI via subprocess
**Description:** Como usuário, quero instanciar uma engine UCI a partir de um path de executável, para usar engines como Stockfish.

**Acceptance Criteria:**
- [ ] Struct `UciEngine` que implementa trait `Engine`
- [ ] Spawna o processo da engine e comunica via stdin/stdout
- [ ] Implementa handshake UCI: envia `uci`, espera `uciok`
- [ ] Implementa `isready`/`readyok` para sincronização
- [ ] Envia `ucinewgame` no `new_game()` para limpar estado entre partidas
- [ ] Envia `position startpos moves e2e4 e7e5 ...` com histórico completo
- [ ] Envia `go wtime X btime Y winc Z binc W` com informações de relógio
- [ ] O leitor do stdout da engine deve consumir/descartar todas as linhas intermediárias (como `info`) até encontrar e extrair a linha contendo `bestmove`
- [ ] Faz parsing de `bestmove e2e4` da resposta
- [ ] Envia `quit` no drop/quit
- [ ] Timeout configurável para respostas da engine
- [ ] Testes com mock ou engine simples (se disponível no ambiente)

### US-011: Orquestrador de partidas (Match)
**Description:** Como usuário, quero executar uma partida completa entre duas engines, para comparar engines ou testar a lib.

**Acceptance Criteria:**
- [ ] Struct `Match` que recebe duas `Box<dyn Engine>`, um `Game` e configuração de relógio
- [ ] Loop de jogo: pede move à engine do lado a mover, aplica no Game, verifica fim de jogo, passa para a próxima engine
- [ ] Atualiza o relógio a cada movimento
- [ ] Termina em: checkmate, stalemate, draw (qualquer tipo), flag (tempo esgotado), ou engine error
- [ ] Retorna `GameResult` ao final
- [ ] Callback ou trait para observar a partida (e.g. printar cada movimento)

### US-012: Binário chess-runner
**Description:** Como usuário, quero um CLI que execute uma partida entre duas engines UCI, para testar a integração completa.

**Acceptance Criteria:**
- [ ] Aceita argumentos: paths das duas engines, time control (tempo + incremento)
- [ ] Instancia duas `UciEngine`, cria um `Match`, executa a partida
- [ ] Printa cada movimento em notação UCI e o board (debug format) durante a partida
- [ ] Printa o resultado final (quem venceu, motivo do empate, ou erro)
- [ ] Suporta match de N jogos (`--games N`), alternando cores entre as engines a cada jogo
- [ ] Aceita argumento opcional `--start-fen "..."` para iniciar todas as partidas do match a partir de uma posição específica, garantindo variabilidade em matches longos
- [ ] Ao final do match, printa score total (vitórias/derrotas/empates por engine)
- [ ] `cargo run -p chess-runner -- --engine1 /path/to/stockfish --engine2 /path/to/engine2 --time 60000 --inc 1000 --games 10 --start-fen "rnbqkbnr/..."`

## Functional Requirements

- FR-1: O `Cargo.toml` raiz deve definir um workspace; `chesslib` e `chess-runner` devem ser members
- FR-2: Toda a lógica de xadrez (board, movegen, game, UCI, engine) fica em `chesslib`
- FR-3: `Board` deve expor informação suficiente para detectar todas as condições de fim de jogo
- FR-4: `Game` é a struct central para partidas — encapsula `Board`, histórico, relógio e resultado
- FR-5: `Game::make_move()` deve rejeitar movimentos ilegais com erro descritivo
- FR-6: Draws automáticos (fivefold repetition, 75-move rule) devem ser aplicados sem claim
- FR-7: Threefold repetition é aplicado automaticamente pelo `Match`; 50-move rule exposto como consulta mas também aplicado automaticamente no `Match`
- FR-8: O parsing UCI de moves deve ser robusto: rejeitar strings mal formadas com erro claro
- FR-9: `UciEngine` deve lidar com engines que crasham ou não respondem (timeout)
- FR-10: O relógio deve usar `std::time::Instant` para medir tempo real gasto por movimento
- FR-11: O `Match` deve ser determinístico dado o mesmo input das engines (sem race conditions)

## Non-Goals

- Parsing de linhas `info` da UCI (depth, score, pv, etc.) — fora do escopo
- Implementação de UCI `option`/`setoption` — engines usam configuração padrão
- GUI ou interface web
- Suporte a variantes de xadrez (Chess960, etc.)
- Rating/ELO calculation
- Opening book ou tablebase integration
- PGN parsing/export (pode ser adicionado futuramente)
- Pondering (`go ponder`)

## Technical Considerations

- Usar Zobrist hashing desde o início para detecção de repetição de posição (gerar chaves aleatórias no build.rs ou em constantes)
- `Engine` trait é async — usar `tokio` como runtime. `chesslib` terá `tokio` como dependência para o módulo de engine/UCI
- `UciEngine` usa `tokio::process::Command` para I/O assíncrono com o subprocess da engine
- O `Clock` precisa de precisão de milissegundos para compatibilidade com UCI (wtime/btime são em ms)
- O build.rs existente e a geração de lookup tables devem permanecer inalterados na migração para workspace
- Threefold repetition é aplicado automaticamente pelo `Match` (draw automático, não requer claim)
- `chess-runner` suporta partidas em série (match de N jogos) na primeira versão

## Success Metrics

- Todos os testes de perft existentes continuam passando após a reestruturação
- Uma partida completa entre duas instâncias de Stockfish roda sem erros via `chess-runner`
- Detecção de fim de jogo cobre todos os cenários FIDE testáveis
- Move parsing UCI roundtrip: `ChessMove -> UCI string -> ChessMove` preserva o move original

## Decisions

- **Zobrist hashing** desde o início para detecção de repetição
- **Engine trait async** com `tokio` como runtime
- **Threefold repetition automático** — aplicado pelo `Match`, sem necessidade de claim
- **Match de N jogos** suportado no `chess-runner` na primeira versão
