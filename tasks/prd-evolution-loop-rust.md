# PRD: evolution-loop — Reescrita em Rust

## Introduction

Reescrever o orquestrador de evolução do engine Wiggum (`scripts/evolution-loop.sh`) como uma nova crate Rust `evolution-loop` no workspace. O script de shell atual (~2630 linhas de bash + Python inline) sofre de bugs de lógica em casos extremos, travamentos sem timeout, logs insuficientes e alta dificuldade de manutenção. A nova implementação em Rust mantém o mesmo esquema de fases (propose → implement → validate → benchmark → decide) e a mesma estrutura de artefatos JSON/markdown, mas com tipagem forte, tratamento de erros explícito, logging estruturado e timeouts configuráveis.

## Goals

- Substituir completamente `scripts/evolution-loop.sh` por um binário Rust `evolution-loop`
- Preservar a estrutura de artefatos de sessão (`session.env`, `iteration.json`, arquivos markdown por fase) para não quebrar skills Claude existentes
- Adicionar timeouts por fase para eliminar travamentos infinitos
- Estruturar logs com nível (INFO/WARN/ERROR) e timestamps, com modo verbose que mostra stdout/stderr do processo Claude em tempo real
- Usar `serde_json` para toda leitura/escrita de JSON; shell-out para `git` e `cargo`
- Tornar bugs de lógica detectáveis em tempo de compilação via tipos e enums de estado

## User Stories

### US-001: Nova crate `evolution-loop` no workspace
**Description:** As a developer, I want a `evolution-loop` Rust crate added to the Cargo workspace so that the orchestrator can be built and tested like any other crate.

**Acceptance Criteria:**
- [ ] Diretório `evolution-loop/` criado com `Cargo.toml` válido (name = "evolution-loop", binary target `evolution-loop`)
- [ ] Crate adicionada ao `members` em `Cargo.toml` raiz do workspace
- [ ] `cargo build -p evolution-loop` compila sem erros
- [ ] Dependências mínimas: `serde`, `serde_json`, `clap`, `anyhow`, `chrono`, `tracing`, `tracing-subscriber`
- [ ] Typecheck/lint passa (`cargo clippy -p evolution-loop`)

### US-002: Interface de linha de comando
**Description:** As an operator, I want a clear CLI to start new sessions and resume interrupted ones.

**Acceptance Criteria:**
- [ ] Subcomando `start` aceita: `--baseline-version <tag>` (obrigatório), `--ideas-file <path>` (opcional), `--output-dir <path>` (padrão: `tasks/evolution-runs`), `--max-iterations <n>` (padrão: 10), `--max-infra-failures <n>` (padrão: 3), `--phase-timeout-secs <n>` (padrão: 1800), `--verbose`
- [ ] Subcomando `resume` aceita: `--session <path>` (obrigatório), `--from <phase>` (opcional: propose/implement/validate/benchmark/decide), `--phase-timeout-secs <n>`, `--verbose`
- [ ] `--help` imprime uso e sai com código 0
- [ ] Argumentos inválidos imprimem mensagem de erro descritiva e saem com código diferente de 0
- [ ] `cargo build -p evolution-loop` e `./target/debug/evolution-loop --help` funcionam

### US-003: Leitura e escrita de estado da sessão (`session.env` e `iteration.json`)
**Description:** As a developer, I want strongly-typed Rust structs for all session/iteration state so that JSON manipulation bugs are caught at compile time.

**Acceptance Criteria:**
- [ ] Struct `SessionMetadata` (derivando `Serialize`/`Deserialize`) cobre todos os campos de `session.env` atuais
- [ ] Struct `IterationState` (derivando `Serialize`/`Deserialize`) cobre todos os campos de `iteration.json` atuais, incluindo `ideas`, `candidate`, `correctness`, `stockfishComparison`, `stateMachine`, `artifacts`, `decision`
- [ ] Enum `IterationPhase` e enum `IterationOutcome` com todos os estados válidos
- [ ] Transições de estado validadas: tentar avançar de um estado inválido retorna `Err` em vez de corromper o arquivo
- [ ] Testes unitários cobrem serialização/desserialização de `IterationState` com um JSON de exemplo real

### US-004: Gerenciamento de worktrees de candidatos
**Description:** As the orchestrator, I want candidate git worktrees to be created and cleaned up reliably so that interrupted sessions don't leave orphaned worktrees.

**Acceptance Criteria:**
- [ ] Criação de worktree: shell-out para `git worktree add --detach <dir> <ref>` e `git checkout -b <branch>`
- [ ] Remoção de worktree: shell-out para `git worktree remove --force <dir>` seguido de `rm -rf <dir>`; sem panic se o diretório não existir
- [ ] Nome de branch segue o padrão `wiggum-evolution/<session-id>/iteration-<n>`
- [ ] Se a criação falhar, `iteration.json` é preenchida com `state: "failed"` e a iteração é contada como infra-failure
- [ ] Worktree órfão deixado por iteração interrompida é removido no início do próximo `resume`

### US-005: Gate de correção (validate phase)
**Description:** As the orchestrator, I want a correctness gate that runs `cargo build` and `cargo test` in the candidate workspace and records results before benchmarking.

**Acceptance Criteria:**
- [ ] Executa `cargo build --workspace` no diretório do worktree candidato
- [ ] Executa `cargo test --workspace -- --skip gen_files::magics::name` no diretório do worktree candidato
- [ ] Cada comando tem timeout configurável (usa `--phase-timeout-secs` ou um timeout fixo para o gate de correção)
- [ ] `correctness/results.md` e o campo `correctness` em `iteration.json` são preenchidos com status (passed/failed) e resultado de cada check
- [ ] Se algum check falhar, `iteration.json` vai para `state: "failed"`, benchmark é marcado como `skipped` e o worktree é removido

### US-006: Execução de fases Claude com timeout
**Description:** As the orchestrator, I want each Claude skill phase to run with a configurable timeout so that hung processes don't block the session forever.

**Acceptance Criteria:**
- [ ] Cada fase chama `openclaude --dangerously-skip-permissions --add-dir <session_dir> --add-dir <repo_root> --print /<skill>` no diretório do worktree candidato
- [ ] O processo é morto se exceder `--phase-timeout-secs` segundos; a iteração é marcada como `failed` com razão "phase timeout"
- [ ] Em modo verbose (`--verbose`): stdout/stderr do processo Claude são exibidos em tempo real via `tee` para o log do arquivo
- [ ] Em modo silencioso: stdout/stderr são capturados apenas para o log de fase (`phase-logs/<phase>.log`)
- [ ] O binário Claude a usar é configurável via variável de ambiente `CLAUDE_BIN` (padrão: `openclaude`)
- [ ] Exit code diferente de zero do Claude é registrado como falha de fase com o log relevante referenciado

### US-007: Loop principal de iterações
**Description:** As an operator, I want the main loop to run up to `--max-iterations` iterations with automatic stopping conditions so that the session terminates gracefully.

**Acceptance Criteria:**
- [ ] Loop itera de 1 a `--max-iterations` executando a sequência completa de fases por iteração
- [ ] Para automaticamente quando: (a) max iterations atingido, (b) max infra failures atingido, (c) fase propose sinaliza `no_hypothesis`, (d) estado de iteração inesperado
- [ ] `STOP_REASON` e `STOP_REASON_DETAILS` são gravados em `summary.md` ao final
- [ ] Ao final de iteração `accepted`: incrementa infra-failure-count somente se a promoção falhou; zera o contador em sucesso
- [ ] Ao final de iteração `rejected` ou `inconclusive`: marca ideia como usada no checklist (se aplicável); reseta infra-failure-count
- [ ] Ao final de iteração `failed`: incrementa infra-failure-count

### US-008: Promoção de candidatos aceitos
**Description:** As the orchestrator, I want accepted candidates to be atomically promoted as the new baseline so that subsequent iterations use the correct baseline binary.

**Acceptance Criteria:**
- [ ] Copia binário candidato para `chess-engine/versions/<promoted-version>/wiggum-engine`
- [ ] Atualiza `session.env` com `active_baseline_*` e `accepted_baseline_*` apontando para o novo artefato
- [ ] Atualiza campos correspondentes em `iteration.json.decision.promotion`
- [ ] Se a cópia do binário falhar, a iteração é reclassificada como `failed` e o baseline anterior é mantido
- [ ] Versão promovida é calculada: bump minor para `self_proposed`, bump major para `user_ideas_file`

### US-009: Marcação de ideias usadas no checklist
**Description:** As the orchestrator, I want the ideas file checklist to be updated after each tested iteration so that ideas aren't retried.

**Acceptance Criteria:**
- [ ] Após iteração com `proposalSource = "user_ideas_file"` atingir estado terminal (accepted/rejected/inconclusive/failed), a entrada correspondente no checklist é alterada de `- [ ]` para `- [x]`
- [ ] Se a entrada não for encontrada, a iteração é reclassificada como `failed` e uma mensagem de erro é registrada
- [ ] Após a marcação, `ideas_file_pending_count` em `session.env` é recalculado
- [ ] Se o arquivo de ideias ficar sem entradas pendentes, `IDEAS_FILE_RESOLVED` é limpo para que a próxima iteração use o fluxo self-propose

### US-010: Resumo de sessão e relatório final
**Description:** As an operator, I want a human-readable `summary.md` written at the end of every session so that I can audit what happened.

**Acceptance Criteria:**
- [ ] `summary.md` inclui: session id, baseline inicial, baseline final aceito, max iterations, iterações completadas, stop reason, lista de versões aceitas, lista de versões rejeitadas, tabela de artefatos por iteração
- [ ] Escrito ao final da sessão, independente do stop reason
- [ ] Escrito mesmo se o processo for interrompido por Ctrl-C (signal handler registra o summary parcial)

### US-011: Modo resume
**Description:** As an operator, I want to resume an interrupted session at a specific phase so that I don't have to restart from scratch.

**Acceptance Criteria:**
- [ ] `evolution-loop resume --session <path>` detecta a iteração mais recente e a fase em que parou via `iteration.json.state`
- [ ] `--from <phase>` permite forçar início de fase (útil se o estado ficou inconsistente)
- [ ] Valida que o worktree candidato ainda existe e que os binários baseline apontados em `iteration.json` ainda existem
- [ ] Se o worktree foi perdido, registra falha e pula para a próxima iteração de sessão normal
- [ ] Sessão resume aceita `--phase-timeout-secs` e `--verbose` para ajustar o comportamento do processo retomado

### US-012: Logging estruturado
**Description:** As a developer debugging a hang, I want timestamped structured logs so that I can see exactly what the orchestrator is doing and where it stopped.

**Acceptance Criteria:**
- [ ] Todos os eventos relevantes usam `tracing` com nível apropriado (INFO para progresso normal, WARN para falhas não-fatais, ERROR para falhas fatais)
- [ ] Cada linha de log inclui timestamp ISO-8601 e o nome da fase/iteração atual como campos de contexto
- [ ] Em modo silencioso: logs vão apenas para stdout do processo orquestrador (não misturado com saída do Claude)
- [ ] Em modo verbose: logs do orquestrador são prefixados claramente para distinguir de output do Claude
- [ ] Início e fim de cada fase são sempre registrados com nível INFO, incluindo o exit code do processo

## Functional Requirements

- FR-1: O binário `evolution-loop` deve ser adicionado ao workspace como crate `evolution-loop/`
- FR-2: Subcomandos `start` e `resume` com flags documentadas em `--help`
- FR-3: Toda manipulação de JSON via `serde_json` com structs tipados — sem Python inline, sem `sed`, sem `awk`
- FR-4: Shell-out para `git` e `cargo` usando `std::process::Command` com captura de stdout/stderr
- FR-5: Cada fase Claude tem timeout configurável; processo filho é terminado com `SIGKILL` ao expirar
- FR-6: Estrutura de artefatos de sessão idêntica ao script de shell (compatibilidade com skills Claude existentes)
- FR-7: Logging via `tracing`/`tracing-subscriber` com timestamps, níveis e campos de contexto de iteração/fase
- FR-8: Modo verbose faz `tee` do stdout/stderr do Claude para terminal e para o log de fase simultaneamente
- FR-9: Signal handler para SIGINT/SIGTERM grava o summary parcial antes de sair
- FR-10: `CLAUDE_BIN` como variável de ambiente para o binário Claude (padrão: `openclaude`)

## Non-Goals

- Não migrar as skills Claude (`evolution-propose`, `evolution-implement`, etc.) — elas permanecem como estão
- Não alterar o formato de `iteration.json` ou `session.env` — compatibilidade total com skills existentes
- Não implementar a lógica de benchmark ou match de chess dentro desta crate — isso é responsabilidade das skills
- Não reescrever o script `benchmark-version.sh` ou outros scripts auxiliares
- Não adicionar UI web ou TUI — saída em texto simples para terminal
- Não usar a crate `git2` (libgit2) — apenas shell-out para `git` para manter dependências simples

## Technical Considerations

- **Workspace**: adicionar `evolution-loop` ao `members` em `Cargo.toml` raiz. A crate tem apenas um binary target.
- **Dependências sugeridas**: `clap` (CLI), `serde`/`serde_json` (JSON), `anyhow` (erros), `chrono` (timestamps/session IDs), `tracing`/`tracing-subscriber` (logging), `ctrlc` (signal handler para SIGINT)
- **Shell-out para git/cargo**: usar `std::process::Command::new("git")` / `Command::new("cargo")` com `.current_dir()` e `.stdout(Stdio::piped())` — nunca invocar via `sh -c`
- **Timeout de processo**: usar `std::thread` com `child.wait_timeout()` ou `child.kill()` após deadline; alternativa: crate `wait-timeout`
- **Modo verbose tee**: spawnar thread para ler stdout/stderr do Claude e escrever simultaneamente em `File` e `Stdout`
- **Compatibilidade de artefatos**: escrever `session.env` como pares `key=value` (sem aspas) exatamente como o shell script; escrever `iteration.json` com indentação de 2 espaços e newline final
- **Inicialização de sessão**: gerar session ID como `YYYYMMDDTHHMMSSz` via `chrono::Utc::now().format(...)`
- **Validação de versão**: aceitar apenas tags no formato `v<major>.<minor>` (ex: `v1.2`)

## Success Metrics

- Zero travamentos de processo — toda fase tem timeout explícito
- Erros de lógica de transição de estado são detectáveis em compilação via enum `IterationOutcome`
- Tempo de manutenção reduzido: um único arquivo Rust tipado em vez de 2600 linhas de bash+Python
- Logs suficientes para diagnosticar onde qualquer sessão parou sem precisar de debugger

## Open Questions

Resolvidas:

- **`no_hypothesis`**: encerra a sessão imediatamente (comportamento atual mantido).
- **Artefatos adicionais em `session.env`**: descobrir e documentar durante a implementação lendo as skills Claude existentes.
- **Script de shell**: deletar `scripts/evolution-loop.sh` após o binário Rust estar funcionando.
