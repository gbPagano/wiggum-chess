#import "../lib.typ": ieee

#let titulo = "Meta-otimização do ajuste heurístico em arquiteturas Minimax utilizando LLM Agent Loops."
#let authors = ("Guilherme Borges Pagano", "João Marcos de Oliveira Calixto")
#let data = "16 de junho de 2025"

#show: ieee.with(
    title: titulo,
    abstract: [
    ],
    authors: (
        (
            name: "Guilherme Borges Pagano",
            department: [Faculdade de Engenharia Elétrica],
            organization: [Universidade Federal de Uberlândia],
            location: [Uberlândia, Brasil],
            email: "guilhermebpagano@ufu.br"
        ),
        (
            name: "João Marcos de Oliveira Calixto",
            department: [Faculdade de Engenharia Elétrica],
            organization: [Universidade Federal de Uberlândia],
            location: [Uberlândia, Brasil],
            email: "jm.calixto@ufu.br"
        ),
        (
            name: "Augusto W. F. Veloso da Silveira",
            department: [Faculdade de Engenharia Elétrica],
            organization: [Universidade Federal de Uberlândia],
            location: [Uberlândia, Brasil],
            email: "augustofleury@ufu.br"
        )
    ),
    index-terms: ("Inteligência Artificial", "Xadrez", "Meta-otimização", "Agent Loop;"),
    bibliography: bibliography("refs.yml", title: "Referências", style: "ieee"),
)

= Introdução

O xadrez constitui um problema canônico para a inteligência artificial e a teoria da decisão adversarial. Formalmente classificado como um jogo determinístico, de informação perfeita, finito e de soma zero para dois jogadores @neumann1944, seu espaço de estados é computacionalmente colossal: a árvore de jogo possui um limite inferior estimado em $10^120$ permutações únicas, o chamado Número de Shannon @shannon1950. Essa explosão combinatória inviabiliza a busca exaustiva desde a posição inicial até os estados terminais, tornando obrigatório o uso de mecanismos de aproximação e poda.

A arquitetura clássica para superar essa barreira combina o algoritmo Minimax com a poda alfa-beta @minimax_wiki @alphabeta_wiki. O Minimax explora a árvore de jogo assumindo que ambos os jogadores atuam de forma ótima, enquanto a poda alfa-beta elimina ramos que não podem influenciar a decisão final, reduzindo drasticamente o número de nós avaliados, conforme analisado formalmente por @knuth1975. Contudo, como a profundidade da busca é limitada por horizonte computacional, a avaliação de posições não-terminais depende de uma função heurística de avaliação estática – tipicamente uma combinação linear de material, tabelas posicionais (piece-square tables) @pst_wiki, mobilidade e outros termos de conhecimento de xadrez.

A calibração dessa função heurística é um dos pontos mais trabalhosos e frágeis do desenvolvimento de engines de xadrez. O ajuste manual de pesos, matrizes e até mesmo a introdução ou remoção de features exige extensivas rodadas de partidas contra versões anteriores ou engines de referência. O problema é agravado pelo fato de que o “sinal” de qualidade é observado por resultados de partidas – uma medida ruidosa, sujeita à variância de abertura, controle de tempo e sorte residual. Métodos estatísticos (como otimização estocástica e abordagens bayesianas) automatizam parte desse processo, mas ainda operam dentro de um espaço paramétrico pré-definido, raramente alterando a estrutura do código heurístico.

Nos últimos anos, a engenharia de software passou a documentar evidências de que grandes modelos de linguagem (LLMs) conseguem melhorar código iterativamente quando existe um juiz verificável – tipicamente um compilador, uma suíte de testes ou um harness de validação executável. O benchmark SWE-bench formaliza essa avaliação por patches verificados por testes em repositórios reais @jimenez2024, e sistemas agentes como SWE-agent mostram que feedback ambiente e validação por testes aumentam a taxa de resolução sem necessidade de retreinar os pesos do modelo @swe_agent2024. Paralelamente, frameworks que acoplam LLMs a avaliadores sistemáticos têm evoluído programas em direção a melhores scores em domínios matemáticos e de otimização – como FunSearch na descoberta de soluções em matemática discreta @funsearch2023 e SBLLM na otimização de código guiada por busca e refinamento iterativo @sbllm2024.

A hipótese central deste artigo é que essa lógica de gerar → verificar → corrigir pode ser transplantada para o ajuste tático de heurísticas em engines clássicas de xadrez. Em vez de ajustar pesos “à mão” ou exclusivamente por otimização estocástica, é proposto um framework de meta-otimização onde um script orquestrador persistente submete o módulo de avaliação a um LLM e valida as mutações através de feedback verificável do compilador, de testes de regressão e de um avaliador de desempenho (ex.: acurácia em posições táticas ou resultado contra um motor de referência). O ajuste heurístico passa a ser tratado como um problema de busca em espaço de programas, análogo às abordagens recentes que combinam LLMs com avaliadores sistemáticos.

= Trabalhos Relacionados

Esta seção contextualiza a proposta frente a três linhas de pesquisa: 

    +   O ajuste automático de heurísticas e parâmetros em motores de xadrez clássicos; 
    +   O uso de grandes modelos de linguagem (LLMs) para geração, correção e otimização autônoma de código;
    +   Arquiteturas de agentes com loops de feedback verificável e iteração contínua. 
    
A síntese dessas vertentes delimita a lacuna explorada pelo artigo: a meta-otimização de funções de avaliação heurística em motores Minimax por meio de um agente LLM em loop, com validação por compilação, testes e desempenho tático.

== Ajuste automático de heurísticas e parâmetros em motores clássicos

A literatura de tuning de motores de xadrez trata explicitamente o problema como otimização sob ruído, pois a função objetivo (força de jogo) é observada por resultados de partidas – medida com variância alta, dependente de aberturas, controle de tempo e sorte residual. Nesse cenário, o método SPSA (Simultaneous Perturbation Stochastic Approximation) é frequentemente utilizado por sua eficiência em espaços de alta dimensionalidade, estimando o gradiente com poucas avaliações por iteração @spall1992. Há também propostas recentes que exploram inferência bayesiana para melhorar a eficiência e estabilidade do processo, mantendo a avaliação por confronto entre variantes @ivec2022.

Além disso, práticas difundidas na comunidade – embora nem sempre formalizadas em papers – incluem métodos de regressão logística para calibrar pesos de avaliação a partir de posições rotuladas por resultado ou score, conhecido como Texel Tuning @texel_tuning_wiki. Esses métodos reforçam que o ajuste “automático” já é parte do estado da prática, mas tipicamente restrito a vetores de parâmetros em um modelo heurístico pré-definido, em vez de alterações estruturais no código ou introdução de novas features.

== LLMs para geração, correção e otimização autônoma de código

Em engenharia de software, a virada metodológica recente está na avaliação por artefatos verificáveis. O benchmark SWE-bench define tarefas reais extraídas de issues e pull requests em repositórios, julgando a correção pela execução de testes @jimenez2024. Esse desenho aproxima a avaliação do que um desenvolvedor faria: modificar código e validar por suíte de testes. Sobre esse benchmark, sistemas agentes interativos como SWE-agent argumentam que o desempenho depende criticamente do “acoplamento” agente–ambiente: ações de navegação/edição e feedback conciso por turno melhoram a capacidade de iterar até passar nos testes @swe_agent2024.

Em paralelo, a subárea de reparo automatizado de programas (Automated Program Repair, APR) documenta um padrão convergente: LLMs podem ser integrados a ciclos generate-and-validate, mas ganhos importantes aparecem quando o loop incorpora feedback de falhas de teste (ou de compilação) para guiar novas tentativas, como em abordagens conversacionais que intercalam geração e feedback ao invés de apenas amostrar múltiplos patches independentes @acm_icse2023. Linhas próximas também exploram feedback de compilador para reduzir erros de contexto e de uso de APIs em geração de código em nível de projeto @compiler_feedback2024.

== Arquiteturas de agentes com loops de feedback verificável e iteração contínua

O deslocamento de “uma chamada do LLM” para “um agente em loop” é sustentado por frameworks de raciocínio e ação e de auto-refinamento. ReAct formaliza a alternância entre raciocínio e ações no ambiente @react2022; Self-Refine modela iteração crítica/refinamento @self_refine2023; Reflexion propõe incorporar feedback em “memória” textual para melhorar tentativas subsequentes sem ajuste de pesos @reflexion2023. Frameworks de deliberação como Tree of Thoughts generalizam a ideia para exploração de múltiplos caminhos, aproximando o processo de uma busca em espaço de soluções @tree_of_thoughts2023.

Em termos de infraestrutura, plataformas como OpenHands sistematizam a interação de agentes com repositórios, terminal e benchmarks, reforçando a noção de “ambiente verificável” @openhands2024. Um padrão de engenharia particularmente relevante para este artigo é o Ralph – um loop externo persistente no qual cada iteração executa uma unidade de trabalho, e um verificador decide se a tarefa terminou; caso contrário, injeta feedback e o agente tenta novamente @ralph_loop, @ralph_wiggum_ai.

== Síntese e a lacuna explorada

A lacuna explorada por este artigo não é “usar LLMs para jogar xadrez”, mas usar LLMs para otimizar o código de um motor clássico – em especial sua função de avaliação heurística. Isso conecta diretamente (i) tuning sob ruído (partidas), (ii) geração/reparo de código com validação por testes/compilação, e (iii) loops agentes com avaliação externa. Trabalhos como FunSearch mostram que emparelhar LLM com avaliadores sistemáticos permite evoluir programas rumo a melhores scores @funsearch2023; SBLLM mostra que otimização de código pode ser formulada “como busca” com refinamento iterativo guiado por execução @sbllm2024; e abordagens recentes de cooperação compilador–LLM explicitam validação de correção como parte do loop @compiler_feedback2024.

Diferentemente desses trabalhos, o foco atual recai sobre um domínio adversarial de busca (Minimax + poda alfa-beta), onde o feedback de desempenho é inerentemente ruidoso e o espaço de modificações inclui tanto parâmetros contínuos quanto alterações estruturais no código da heurística.

= Arquitetura

Este artigo propõe um sistema de meta-otimização organizado em dois níveis acoplados: um loop interno de busca adversarial (Minimax com poda alfa-beta) e um loop externo persistente no qual um agente baseado em LLM modifica o código da função de avaliação heurística, verifica a correção (compilação + testes) e mensura o ganho de desempenho (acurácia tática ou força relativa). A seguir, é detalhado cada componente e as decisões de projeto que viabilizam o ajuste heurístico como um problema de busca em espaço de programas.

== Acoplamento de Loops

A arquitetura central pode ser apresentada como um acoplamento entre:

=== Loop interno (busca adversarial): 

Implementado pela engine ChessLib @chesslib2026, o algoritmo Minimax com poda alfa-beta explora a árvore de jogo até uma profundidade limite, utilizando uma função de avaliação estática $E(s)$ para posições não-terminais. Essa função combina material, tabelas posicionais (piece-square tables) e, opcionalmente, outros termos heurísticos. O loop interno opera sobre um estado do tabuleiro e retorna o melhor lance encontrado.

=== Loop externo (meta-otimização): 

Opera sobre o artefato “código/heurísticas” (parâmetros, matrizes, features) e busca melhorias mensuráveis em um critério objetivo. O gerador é um LLM que propõe mutações no código; o avaliador é composto por (i) compilador (verificação sintática), (ii) testes de regressão (validação funcional) e (iii) benchmark tático ou partidas (medição de desempenho). O loop externo trata o ajuste heurístico como um problema de busca em espaço de programas, analogamente a frameworks que combinam LLMs com avaliadores sistemáticos @funsearch2023, @sbllm2024.
    
Essa separação é coerente com a visão clássica de “busca + avaliação” em jogos determinísticos: o loop interno estima o valor posicional sob um horizonte, enquanto o loop externo ajusta como essa estimativa é produzida, mantendo a engine como um sistema determinístico verificável por compilação e testes.

== Loop externo com Ralph

Para implementar o loop externo de forma robusta e autônoma, adotamos o padrão Ralph – um loop persistente no qual cada iteração executa uma unidade de trabalho e um verificador decide se a tarefa foi concluída; caso contrário, injeta feedback e o agente tenta novamente @ralph_loop, @ralph_wiggum_ai. No contexto deste artigo, a sequência de tarefas de otimização é gerada e executada sob essa técnica, cujo nome é inspirado no personagem de The Simpsons e popularizado como um padrão de “autonomia contínua”.

Operacionalmente, o loop externo segue estas etapas:

=== Plano

O LLM (ou um orquestrador) produz uma micro-tarefa – por exemplo, “ajustar pesos da tabela posicional do cavalo no meio-jogo”, “refatorar termo de segurança do rei” ou "inserir feature de mobilidade com custo $O(1)$".

=== Execução

A tarefa é aplicada ao código (edição de arquivos fonte).

=== Verificação

    -   Compilação (garantir que o código permanece sintaticamente correto). 
    -   Execução de testes de regressão (evitar quebras funcionais).
    -   Avaliação de desempenho (acurácia em suíte tática ou partidas contra baseline).

=== Decisão

Se todos os critérios forem satisfeitos, a mudança é “promovida” (commit/merge). Caso contrário, o feedback (erros de compilação, falhas em testes, ou ausência de melhoria) é realimentado ao LLM para uma nova tentativa.

== Interface agente–repositório e feedback verificável

A literatura recente sobre agentes de software sugere que confiabilidade e desempenho dependem de restringir e instrumentar a interação com o ambiente. O SWE-agent introduz o conceito de agent-computer interface (ACI) para oferecer um conjunto pequeno de ações (ver arquivo, buscar, editar trechos) e retornar feedback conciso e estruturado a cada turno @swe_agent2024. Essa ideia é diretamente transferível ao nosso contexto: em vez de permitir que o LLM edite qualquer coisa sem controle, a meta-otimização limita as mudanças ao módulo de avaliação/heurísticas, exige formatação específica (ex.: diffs ou blocos de código) e compila a cada commit.

Do lado do compilador, trabalhos que usam feedback de compilação/execução para orientar otimização mostram que “compilável” e “correto” podem ser tratados como restrições duras no loop, e que a retroalimentação do toolchain pode ser incorporada como sinal para nova tentativa, em vez de apenas amostragem independente @compiler_feedback2024.

Na atual implementação, o agente interage com um repositório Git local que contém o código fonte da engine ChessLib. Cada tentativa de modificação gera um branch temporário, dispara o pipeline de compilação e testes, e registra as métricas de desempenho. O feedback textual (erros do compilador, assertions falhas, acurácia tática) é enviado ao LLM para subsidiar a próxima iteração.

== Critério de aceitação: melhoria relativa contra baseline e motor de referência

A forma mais defensável de definir “melhoria” no loop externo é por comparação pareada com a versão anterior (baseline) sob condições idênticas. Isso espelha a metodologia de testes A/B amplamente utilizada no desenvolvimento de motores de xadrez. O ecossistema do Stockfish e seu sistema de testes Fishtest explicitam o uso de testes sequenciais (SPRT/GSPRT) para decidir se uma mudança é melhor que outra com economia de recursos @stockfish_fishtest. No presente artigo, define-se dois critérios complementares:

    +   Acurácia tática: ganho positivo na suíte de posições (WAC, Kaufman) com significância estatística (teste de McNemar).
    +   Força em partidas: vitória contra a versão baseline em um número controlado de partidas (ou evidência favorável em teste sequencial).

Formalmente, seja $E_"base"$ a versão original e $E_"novo"$ a versão após uma modificação, define-se:

    -   $Delta"Acc"(T)="Acc"(E_"novo", T)-"Acc"(E_"base", T)$, onde $"Acc"(E,T)$ é a acurácia em uma suíte tática $T$ com tempo fixo por posição;
    -   $Delta"Elo"="elo"(E_"novo")-"elo"(E_"base")$, estimado por partidas pareadas.

A mudança é aceita se $Delta"Acc">0$ com $p<0.05$ (McNemar) ou se o teste sequencial cruzar o limite de aceitação para $Delta"Elo">0$ @sprt_wiki. Rejeita-se caso contrário, e o feedback negativo é utilizado para guiar a próxima tentativa do LLM.

Essa estratégia de aceitação/rejeição, combinada com a persistência do loop Ralph Wiggum, permite que o agente evolua a heurística de forma incremental, evitando regressões e acumulando ganhos verificáveis.

= Implementação



= Avaliação Experimental



= Resultados



= Discussão



= Conclusão  

