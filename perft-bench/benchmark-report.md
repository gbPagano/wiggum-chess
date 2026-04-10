# Relatório de benchmarks do `perft-bench`

## Escopo

Este relatório resume os benchmarks executados com o script `./perft-bench/bench.sh` comparando as implementações:

- `chesslib`
- `chess` crate
- `shakmaty`
- `Stockfish` via UCI (`go perft`)
- `chesslib-simple` (apenas no teste de profundidade 4)
- `python-chess` (apenas no teste de profundidade 4)

Os testes foram executados sobre a posição inicial e sobre três presets clássicos:

- `captures`
- `promotions`
- `kiwipete`

## Observações metodológicas

- Os benchmarks foram medidos com `hyperfine`.
- Nos testes de profundidade 4, os tempos dos binários Rust ficaram abaixo de 5 ms, então o próprio `hyperfine` alertou que essas medições podem ter baixa precisão por causa do overhead do shell.
- A medição do `Stockfish` **não é diretamente comparável** com as crates Rust em processo único. No script atual, cada execução do benchmark do Stockfish inclui:
  - startup do processo
  - handshake UCI (`uci` / `isready`)
  - envio de `position fen ...`
  - envio de `go perft ...`
  - encerramento do processo

  Isso adiciona overhead fixo relevante, especialmente nas profundidades menores.
- `chesslib-simple` e `python-chess` só foram incluídos na rodada da profundidade 4 da posição inicial.

## Resumo executivo

Principais conclusões dos resultados coletados:

1. Entre as bibliotecas Rust testadas in-process, a crate `chess` foi a mais rápida em **todos os cenários medidos**.
2. A `chesslib` ficou consistentemente em segundo lugar, geralmente próxima da `chess`, mas ainda atrás em todas as medições apresentadas.
3. A `shakmaty` ficou sempre atrás de `chesslib` e `chess`, com diferença mais acentuada nos presets mais complexos.
4. O `Stockfish` apareceu mais lento do que as crates Rust em todos os benchmarks, mas esse resultado é fortemente afetado pelo modelo de execução via UCI por processo, então ele deve ser interpretado como **custo de uso externo por comando**, não como velocidade pura de move generation.
5. `chesslib-simple` e `python-chess` ficaram muito atrás das implementações Rust otimizadas, como esperado.

## Resultados detalhados

### 1. Posição inicial

FEN:

```text
rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
```

#### Profundidade 4

| Engine | Tempo médio |
|---|---:|
| `chess` | 1.2 ms |
| `chesslib` | 1.5 ms |
| `shakmaty` | 1.6 ms |
| `Stockfish` via UCI | 161.4 ms |
| `chesslib-simple` | 23.3 ms |
| `python-chess` | 235.0 ms |

Notas:
- Esta rodada tem baixa confiabilidade relativa para os três binários Rust principais por estar abaixo de 5 ms.
- Mesmo assim, a ordenação observada foi `chess` > `chesslib` > `shakmaty`.

#### Profundidade 5

| Engine | Tempo médio |
|---|---:|
| `chess` | 11.9 ms |
| `chesslib` | 14.4 ms |
| `shakmaty` | 15.8 ms |
| `Stockfish` via UCI | 173.8 ms |

Resumo:
- `chess` ficou ~21% mais rápida que `chesslib`.
- `chess` ficou ~33% mais rápida que `shakmaty`.
- O tempo do `Stockfish` ainda é dominado por overhead de processo + protocolo.

#### Profundidade 6

| Engine | Tempo médio |
|---|---:|
| `chess` | 241.9 ms |
| `chesslib` | 259.5 ms |
| `shakmaty` | 350.2 ms |
| `Stockfish` via UCI | 500.9 ms |

Resumo:
- `chess` ficou ~7% mais rápida que `chesslib`.
- `chess` ficou ~45% mais rápida que `shakmaty`.
- `chesslib` manteve-se relativamente próxima da `chess` nesta profundidade.

#### Profundidade 7

| Engine | Tempo médio |
|---|---:|
| `chess` | 6.251 s |
| `chesslib` | 7.467 s |
| `shakmaty` | 9.318 s |
| `Stockfish` via UCI | 9.974 s |

Resumo:
- `chess` ficou ~19% mais rápida que `chesslib`.
- `chess` ficou ~49% mais rápida que `shakmaty`.
- `Stockfish` continuou atrás, mas aqui o overhead fixo de processo pesa proporcionalmente menos do que nas profundidades pequenas.

### 2. Preset `captures`

FEN:

```text
rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8
```

Profundidade 5.

| Engine | Tempo médio |
|---|---:|
| `chess` | 128.1 ms |
| `chesslib` | 142.5 ms |
| `shakmaty` | 249.0 ms |
| `Stockfish` via UCI | 445.1 ms |

Resumo:
- `chess` ficou ~11% mais rápida que `chesslib`.
- `chess` ficou ~94% mais rápida que `shakmaty`.
- `chesslib` mostrou desempenho sólido e ainda relativamente próximo da `chess`.

### 3. Preset `promotions`

FEN:

```text
n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - -
```

Profundidade 6.

| Engine | Tempo médio |
|---|---:|
| `chess` | 177.2 ms |
| `chesslib` | 195.4 ms |
| `shakmaty` | 298.0 ms |
| `Stockfish` via UCI | 570.0 ms |

Resumo:
- `chess` ficou ~10% mais rápida que `chesslib`.
- `chess` ficou ~68% mais rápida que `shakmaty`.
- O padrão observado na posição inicial se manteve.

### 4. Preset `kiwipete`

FEN:

```text
r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -
```

Profundidade 5.

| Engine | Tempo médio |
|---|---:|
| `chess` | 261.5 ms |
| `chesslib` | 282.9 ms |
| `shakmaty` | 537.3 ms |
| `Stockfish` via UCI | 731.7 ms |

Resumo:
- `chess` ficou ~8% mais rápida que `chesslib`.
- `chess` ficou ~105% mais rápida que `shakmaty`.
- `kiwipete` ampliou bastante a distância entre `shakmaty` e as duas implementações mais rápidas.

## Tendências observadas

### Ranking geral observado

Nos cenários medidos, a ordem de desempenho ficou consistentemente assim:

1. `chess`
2. `chesslib`
3. `shakmaty`
4. `Stockfish` via UCI
5. `chesslib-simple` / `python-chess` quando incluídos

### Distância entre `chess` e `chesslib`

A `chesslib` não ficou muito distante da crate `chess`:

- posição inicial, depth 5: ~21% atrás
- posição inicial, depth 6: ~7% atrás
- posição inicial, depth 7: ~19% atrás
- `captures`: ~11% atrás
- `promotions`: ~10% atrás
- `kiwipete`: ~8% atrás

Isso sugere que a `chesslib` já está em uma faixa competitiva, especialmente em posições mais complexas onde a diferença para `chess` ficou menor do que na posição inicial de depth 5.

### Comportamento da `shakmaty`

A `shakmaty` ficou atrás em todos os testes e perdeu mais terreno nas posições táticas/ricas em casos especiais:

- posição inicial, depth 5: ~33% atrás de `chess`
- posição inicial, depth 6: ~45% atrás
- posição inicial, depth 7: ~49% atrás
- `captures`: ~94% atrás
- `promotions`: ~68% atrás
- `kiwipete`: ~105% atrás

### Interpretação do resultado do Stockfish

Os tempos do `Stockfish` não devem ser lidos como “o move generator interno do Stockfish é mais lento”. O benchmark atual mede um fluxo externo via shell + UCI, que inclui overhead de:

- criação do processo
- parsing de protocolo
- comunicação por stdin/stdout
- finalização do processo

Esse formato é útil para comparar o custo fim a fim de chamar um engine externo, mas não para comparar de forma justa a rotina interna de perft contra bibliotecas embutidas em um mesmo processo.

## Conclusões

Com base nos resultados atuais:

- A crate `chess` foi a vencedora em desempenho bruto em todos os cenários testados.
- A `chesslib` ficou consistentemente próxima e em alguns casos relativamente perto, o que indica uma base já competitiva.
- A `shakmaty` mostrou desempenho claramente inferior para este tipo específico de benchmark de perft.
- O `Stockfish` via UCI ficou bem atrás, mas isso reflete sobretudo o overhead do modelo de integração escolhido no script.
- `chesslib-simple` e `python-chess` servem bem como referências de implementações menos otimizadas, e os resultados ficaram alinhados com essa expectativa.

## Próximos passos sugeridos

Se o objetivo for aprofundar a análise, os próximos experimentos mais úteis seriam:

1. Medir também a **correção** lado a lado, registrando os nós retornados por cada engine para cada posição/profundidade.
2. Repetir os benchmarks do `Stockfish` em modo mais controlado, mantendo o processo vivo entre execuções para reduzir o overhead UCI/processo.
3. Adicionar mais posições com alto número de checks, pins, en passant e castling para mapear melhor onde a `chesslib` perde para a crate `chess`.
4. Separar benchmarks de profundidade muito baixa (`depth 4`) dos demais, já que abaixo de 5 ms o ruído do harness cresce bastante.
