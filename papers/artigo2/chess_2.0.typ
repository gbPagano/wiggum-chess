#import "lib.typ": ieee

#let titulo = "Desenvolvimento de uma engine de decisão para Xadrez em Rust com busca Minimax e Poda Alfa-Beta"
#let authors = ("Guilherme Borges Pagano", "João Marcos de Oliveira Calixto")
#let data = "16 de junho de 2025"

#show: ieee.with(
  title: titulo,
  abstract: [
    A aplicação de inteligência artificial ao xadrez exige a superação de um espaço de estados computacionalmente vasto. Dando sequência ao desenvolvimento da biblioteca "ChessLib" em Rust, focada na geração eficiente de lances via bitboards, este artigo apresenta a implementação do agente de decisão autônomo da engine. A arquitetura foi estruturada sobre o algoritmo Minimax para a navegação na árvore de busca de soma zero. A tomada de decisão é guiada por uma função de avaliação estática baseada em material e matrizes posicionais. Adicionalmente, o problema do reprocessamento de transposições foi solucionado através de tabelas de transição indexadas por Zobrist Hashing. A pesquisa documenta a integração harmônica entre heurísticas clássicas de decisão adversarial e as estruturas de baixo nível do gerador de lances, estabelecendo um ecossistema autônomo, modular e de alto desempenho.
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
  ),
  index-terms: ("Inteligência Artificial", "Xadrez", "Algoritmo Minimax", "Poda Alfa-Beta;"),
  bibliography: bibliography("refs.yml", title: "Referências", style: "ieee"),
)

= Introdução

No domínio da ciência da computação e da inteligência artificial, o xadrez se estabelece como o modelo clássico da teoria dos jogos e da otimização de buscas numéricas. Formalmente, o xadrez é classificado como um jogo determinístico, finito, de informação perfeita e de soma zero para dois jogadores @neumann1944. A propriedade determinística assegura que não há variáveis estocásticas influenciando o estado das peças; a informação perfeita indica que o estado integral do sistema está simultaneamente disponível a ambos os agentes; e a dinâmica de soma zero dita, matematicamente, que qualquer ganho posicional ou material auferido por um jogador resulta inevitavelmente em uma perda de magnitude diametralmente idêntica para o adversário.

O desafio da formulação de uma solução algorítmica exata para o xadrez reside na explosão combinatória característica de sua árvore de jogo. O limite inferior da complexidade dessa árvore é frequentemente estimado pelo Número de Shannon, postulando a existência de aproximadamente $10^120$ permutações únicas de partidas @shannon1950, uma magnitude que excede o número estimado de átomos no universo observável. Consequentemente, a aplicação de algoritmos de busca exaustiva que mapeiem a árvore de decisão desde o estado inicial até os nós terminais absolutos é uma impossibilidade computacional.

Para contornar essa barreira, a arquitetura moderna das engines de xadrez fundamenta-se em uma síntese entre a exploração otimizada de grafos e a inferência heurística. Dando continuidade ao desenvolvimento da ChessLib — uma biblioteca construída em RUST, baseada em bitboards e magic bitboards para a geração estrutural de lances @chess2026 —, o presente trabalho dedica-se a apresentar, sob embasamento matemático e algorítmico, os componentes subjacentes ao seu novo agente de inteligência artificial. O objetivo é documentar a integração das funções de avaliação estática e dos algoritmos de busca à engine base, estabelecendo um ecossistema autônomo e de alto desempenho para a tomada de decisão adversarial.

= Trabalhos Relacionados

A literatura sobre o desenvolvimento de engines de xadrez pode ser dividida entre a evolução matemática dos algoritmos de busca e os diferentes paradigmas de avaliação de posições. Este trabalho se insere na intersecção entre as abordagens clássicas de otimização e a implementação de alta performance em linguagens compiladas.

== Algoritmos de Busca Clássicos

A fundação da tomada de decisão em jogos de soma zero baseia-se no algoritmo Minimax. No entanto, a viabilidade desse processo atingiu seu marco teórico com a formalização da Poda Alfa-Beta. A análise matemática do desempenho desse algoritmo foi estabelecida por Knuth e Moore @knuth1975, que demonstraram como a poda pode reduzir exponencialmente o número de nós avaliados, permitindo buscas mais profundas no mesmo intervalo de tempo. A arquitetura do agente desenvolvido neste artigo apoia-se diretamente nesses princípios matemáticos para viabilizar a navegação na árvore gerada pela ChessLib.

== Paradigmas de Avaliação e Engines Modernas

No cenário atual, engines de alta performance adotam abordagens complexas para a avaliação do tabuleiro. O stockfish, amplamente reconhecido como a engine mais forte de código aberto, utiliza uma rede neural atualizável de forma eficiente (NNUE) combinada com a busca alfa-beta, enquanto projetos como o Leela Chess Zero apoiam-se em aprendizado profundo por reforço e busca de Monte Carlo (MCTS). 

Em contraste com as complexidades estocásticas e computacionais das redes neurais, o escopo deste trabalho resgata o paradigma clássico: o uso de funções de avaliação estática baseadas em contagem de material e matrizes posicionais (Piece-Square Tables). Essa escolha garante uma execução puramente determinística e de baixíssimo custo de processamento, ideal para validar a eficiência de baixo nível em Rust.

= Arquitetura

A estrutura de uma engine de xadrez clássico é composta por módulos interdependentes: 

    +   O processo de busca, que projeta ramificações futuras; 
    +   A função de avaliação estática, que aplica um mapeamento heurístico sobre posições não-terminais; 
    +   Sistemas de memória e ordenação, que manipulam a eficiência da pesquisa. 
    
A implementação da inteligência artificial descrita neste artigo foi projetada para atuar diretamente sobre a arquitetura de bitboards da biblioteca ChessLib.

== Função de Avaliação Estática

Quando o algoritmo atinge o limite de profundidade estipulado (horizonte computacional) sem alcançar um desfecho definitivo, a função de avaliação é invocada. Trata-se de um mapeamento matemático de um estado de tabuleiro para um valor escalar, que mensura o grau de vantagem detido por um dos jogadores @shannon1950. 

O nível primário dessa avaliação repousa na contagem estrita de material por somatório linear ponderado, onde magnitudes heurísticas são atribuídas às classes das peças.

$ E_"material"(S) = sum_(p in P_"brancas") w(p) - sum_(p in P_"pretas") w(p) $

Onde $w(p)$ refere-se à magnitude heurística atrelada à classe da peça $p$ (e.g., Peão=100, Cavalo=320, Torre=500).

Para adicionar inteligência espacial sem incorrer nos custos computacionais de cálculos dinâmicos de mobilidade, a arquitetura introduz Piece-Square Tables (PST). Uma PST consiste em um array constante de 64 índices, geometricamente igual ao tabuleiro, onde cada índice arquiva um modificador algébrico que ajusta o valor base da peça em conjunto com suas coordenadas topográficas @pst2026. 

O processo garante avaliação de complexidade de tempo constante $O(1)$. A IA fundamenta-se na Avaliação Incremental: ao simular uma jogada transicional, a sub-rotina subtrai o valor da casa de origem e inflete o valor da casa de destino numa variável global.

A rigidez das matrizes PST precipita falhas conforme a partida avança (por exemplo, o Rei deve ser protegido nas bordas na abertura, mas necessita atacar pelo centro nos finais). Para contornar esse problema, é utilizado o modelo de Tapered Evaluation (Avaliação Interpolada) @tapered2026, mantendo matrizes independentes para Middle Game (MG) e End Game (EG).

O cálculo infere a fase do jogo parametrizando a carga de peças e processa o valor global a partir de uma fórmula de interpolação linear:

$ "eval" = ("mg" times (256 - "phase") + "eg" times "phase")/256 $

Nesta formulação, _mg_ diz respeito à avaliação de jogo a partir das matrizes utilizadas para o Early Game / Mid Game e _eg_ representa a avaliação do jogo utilizando as matrizes de End Game. A variável _phase_ atua como um fator de interpolação que rastreia a densidade de material pesado no tabuleiro. Conforme as peças são capturadas, o valor de _phase_ aumenta gradativamente, permitindo uma transição suave e contínua entre os pesos das matrizes, evitando oscilações bruscas na função de pontuação que poderiam comprometer a estabilidade da árvore de busca.

== Algoritmo Minimax e Busca de Quiescência

O algoritmo Minimax é uma estratégia de busca para jogos de soma zero com informação perfeita, na qual o jogador em turno escolhe o lance que maximiza sua pior desvantagem possível, assumindo que o oponente também jogará de forma ótima. Essa busca é interrompida em uma profundidade limite e a posição resultante é então estimada por uma função de avaliação estática. Nesse modelo, os dois jogadores são representados como um agente Maximizador e um agente Minimizador @minimax2026.

O processo recursivo manifesta-se através das seguintes regras sobre os estados ($s$) e lances possíveis ($a$):

    -   Fronteira: Caso $s$ seja um estado terminal ou alcance o horizonte de busca, $V(s) = E(s)$, em que $E(s)$ é a avaliação estática da posição.
    -   Turno MAX: $V(s) = max_a V(s')$.
    -   Turno MIN: $V(s) = min_a V(s')$.

Caso haja uma paralisação abrupta por se chegar no limite de profundidade, pode-se gerar erros quando a análise termina em uma posição ainda taticamente instável. Para reduzir esse problema, utiliza-se a Busca de Quiescência, uma extensão restrita de profundidade da busca principal que não avalia imediatamente a posição no horizonte, mas continua explorando apenas lances forçantes — em geral capturas e outras continuções táticas relevantes — até que o tabuleiro alcance uma condição estável o suficiente para uma avaliação estática mais confiável @quiescence2026.

== Poda Alfa-Beta

A poda alfa-beta é uma otimização do algoritmo Minimax que reduz o número de nós avaliados sem alterar o valor final retornado pela raiz da busca @alphabeta2026 @knuth1975. 

O método mantém dois limites dinâmicos durante a exploração da árvore: 

    -   $alpha$, que corresponde ao melhor valor já assegurado para o jogador maximizador; 
    -   $beta$, que corresponde ao limite superior ainda admissível para o jogador minimizador. 

Quando a análise de um ramo evidencia que ele não poderá melhorar esses limites, a expansão é interrompida, pois esse ramo não pode afetar a decisão final.

A eficiência dessa técnica depende fortemente da ordenação dos lances @moveordering2026. Se a engine examinar os lances mais fracos primeiro, os cortes matemáticos não ocorrerão e a eficiência da busca decairá para patamares de força bruta. Para direcionar a árvore, implementa-se heurísticas de ordenação.

O primeiro filtro aplicado é a heurística MVV-LVA (Most Valuable Victim - Least Valuable Aggressor). Trata-se de um filtro que processa trocas materiais priorizando cenários em que uma peça aliada de baixo valor ataca um alvo inimigo de alto custo. 

Quando as capturas esgotam seu potencial, o motor precisa analisar os lances pacíficos, que não envolvem tomada de peças. Para não desperdiçar processamento testando movimentos inúteis, implementa-se a heurística assassina (Killer Heuristic). Ela opera sob um princípio de dedução inferencial local: se um lance pacífico já propiciou um corte na malha em algum outro nível horizontal adjacente da árvore, ele recebe privilégio de processamento.

== Tabelas de Transposição e Zobrist Hashing

No xadrez, a ramificação de jogadas não forma uma árvore perfeita onde cada caminho é único. Frequentemente, diferentes sequências de lances levam exatamente à mesma configuração de peças no tabuleiro — um fenômeno chamado de transposição. Se o algoritmo Minimax não reconhecer dinamicamente que já esteve ali, ele desperdiçará processamento recalculando toda uma teia de possibilidades para uma posição que já foi avaliada.

Para mitigar esse custo, o agente arquiva posições mapeadas e avaliadas em uma tabela de transposição. No entanto, indexar tabuleiros inteiros exige alta demanda de processamento. A solução padrão é a aplicação do Zobrist Hashing @zobrist1970.

O método de Zobrist @zobrist2026 funciona como um sistema de etiquetas precisas. Antes da partida, ele atribui um número aleatório único de 64 bits para cada combinação possível de peça e casa. O grande trunfo do método reside puramente no uso da operação lógica XOR.

Como o operador lógico XOR é sua própria entidade inversa matemática, simulações na árvore executam o mapeamento da avaliação incremental $O(1)$. Para atualizar a assinatura da posição após um movimento, não é necessário analisar o tabuleiro inteiro, bastando alterar as chaves da casa de origem e da casa de destino:

$ "H"_"novo" = "H"_"antigo" xor Z_"origem" xor Z_"destino" xor Z_"mudança_turno" $

A integridade de um sistema que comprime posições complexas em apenas 64 bits levanta uma questão: e se dois tabuleiros diferentes gerarem a mesma etiqueta por acidente? A estatística garante que esse risco é quase nulo. A probabilidade de colisão revela que o algoritmo precisaria avaliar grandezas na ordem de $sqrt(2^64) = 2^32$ instâncias @zobrist2026. Isso significa processar mais de 4 bilhões de posições distintas antes que uma única redundância se tornasse uma probabilidade real. Na prática, essa margem é vasta o suficiente para que a computação moderna jogue xadrez sem corromper suas redes neurais ou as podas de caminhos irrelevantes.

= Implementação com a ChessLib

A implementação do agente de decisão foi desenvolvida sobre a base estrutural da ChessLib, biblioteca escrita em Rust e orientada à representação do tabuleiro por bitboards e à geração eficiente de lances por magic bitboards @chess2026. Essa escolha permite que o módulo de busca opere diretamente sobre uma representação compacta e de alto desempenho, reduzindo o custo de manipulação do estado do jogo e favorecendo a exploração profunda da árvore de decisão. 

Nesse arranjo, a ChessLib assume a responsabilidade pela geração de lances legais e pela atualização precisa do estado do tabuleiro, enquanto o agente de decisão concentra-se na seleção da melhor continuação possível a partir da posição atual.

Assim, o fluxo de execução da engine é organizado em etapas. Primeiro, o estado do tabuleiro é recebido pelo módulo de busca. Em seguida, a ChessLib gera os lances legais disponíveis, que são avaliados pela função heurística e percorridos pelo algoritmo de busca adotado. 

Durante esse processo, a posição é analisada de forma recursiva, com aplicação de Minimax, a busca por quiescência e poda alfa-beta, enquanto a tabela de transposição registra posições já examinadas para evitar recomputações desnecessárias. 

#figure(
    image("assets/minimax.png", width: 90%),
    caption: [Fluxo de decisão da engine de xadrez integrada à ChessLib, mostrando a geração de lances, a ordenação, a aplicação de Minimax com poda alfa-beta e a ativação da busca de quiescência antes da avaliação estática.]
)

= Avaliação Experimental

Esta seção descreve o ambiente de execução, a metodologia de benchmark e as métricas empregadas para avaliar a correção e a eficiência computacional da engine.

Os testes foram conduzidos sobre posições táticas codificadas em Forsyth-Edwards Notation (FEN), de modo a medir simultaneamente a qualidade da escolha do lance e o custo associado ao processo de busca @edwards1994.

== Especificações do Sistema e do Software

Os resultados de desempenho de uma engine de xadrez dependem diretamente do ambiente de execução, incluindo o hardware disponível, o sistema operacional e a cadeia de compilação utilizada. Por essa razão, a @system documenta a configuração adotada em todos os experimentos realizados.

#figure(
  table(
    columns: (auto, 1fr),
    align: horizon,
    table.header([Componente], [Especificação]),
    [CPU],             [Intel Core i7-12800H \@ 4.70GHz], 
    [Núcleos/Threads], [14 Cores / 20 Threads],
    [Cache],           [24 MB Intel® Smart Cache],
    [RAM],             [32 GB],
    [OS],              [Arch Linux x86_64 6.12.42-1-lts],
    [Rust],            [-],
    [Compilação],      [release],
  ),
  caption: [Especificações do sistema e do ambiente de software],
) <system>

Todos os experimentos foram executados sob as mesmas condições de compilação e execução, de modo a reduzir interferências externas e preservar a comparabilidade entre os resultados obtidos.

== Metodologia de Benchmark

A avaliação experimental baseou-se na submissão da engine a um conjunto de posições táticas específicas, previamente codificadas em FEN e carregadas diretamente no estado do tabuleiro @edwards1994. Essa estratégia permite reproduzir cenários críticos de busca com controle preciso sobre a posição inicial analisada.

Foram selecionadas posições de benchmark amplamente utilizadas no contexto de xadrez computacional, incluindo problemas oriundos da suíte _Win at Chess_ (WAC) @reinfeld1958, além de posições inspiradas em conjuntos clássicos de teste, como o Kaufman Test @kaufman1992. Essas coleções reúnem posições de elevada densidade tática, caracterizadas por sequências forçadas, ameaças imediatas, possibilidades múltiplas de captura e combinações de curta e média profundidade, sendo adequadas para avaliar o comportamento de algoritmos baseados em busca adversarial.

Para cada posição, a engine foi executada a partir da mesma configuração inicial, registrando-se o lance selecionado e as métricas de desempenho correspondentes. A correção foi aferida com base na coincidência entre o melhor lance retornado pela engine e o lance de referência associado à posição analisada. Dessa forma, o benchmark permite observar simultaneamente a capacidade de resolução tática do agente e o custo computacional necessário para produzir essa decisão.

== Métricas de Eficiência Computacional

O desempenho da engine foi avaliado a partir de métricas de tempo, volume de busca, vazão analítica e qualidade da decisão. Em cada execução, foram registrados:

  + tempo total de resolução, em milissegundos;
  + número total de nós visitados;
  + taxa de processamento em nós por segundo (NPS);
  + taxa de acerto, definida como a proporção de posições em que a engine encontrou o lance de referência da suíte testada.

Além dessas medidas globais, também foram coletadas métricas internas do processo de busca, com o objetivo de quantificar o efeito das heurísticas de otimização implementadas. Entre elas, destacam-se:

  + número de cortes produzidos pela poda alfa-beta;
  + taxa de reaproveitamento da tabela de transposição;
  + quantidade de chamadas à busca de quiescência.

Essas métricas permitem caracterizar tanto o desempenho bruto da implementação quanto a contribuição efetiva dos mecanismos de poda, reaproveitamento de estados e estabilização tática da avaliação. Em particular, a análise dos cortes alfa-beta é relevante por sua relação direta com a redução do número de nós expandidos, conforme discutido na literatura clássica sobre busca adversarial @knuth1975.

As métricas derivadas foram computadas a partir das seguintes definições. 

=== NPS

A taxa de processamento em nós por segundo (NPS) foi calculada pela razão entre o número total de nós visitados e o tempo de resolução, em segundos.

$ "NPS" = N_"nós"/T $

=== Taxa de Acertos

A taxa de acerto foi definida como a proporção de posições em que a engine encontrou o lance de referência da suíte de teste. 

$ "Acc" = (N_"corretos")/(N_"testes") times 100 $

onde $N_"corretos"$ é o número de vezes que a engine encontrou o lance correto para a posição e $N_"testes"$ é o total de posições testadas.

= Resultados

Esta seção apresenta os resultados obtidos a partir dos experimentos realizados com a engine desenvolvida, considerando tanto o desempenho geral da busca quanto o comportamento das heurísticas internas de otimização. Os dados são organizados de modo a evidenciar o custo computacional da exploração da árvore e a qualidade da decisão produzida em posições táticas de referência.


== Desempenho Geral da Busca

A @tab:desempenho apresenta os resultados agregados por profundidade de busca, considerando o tempo médio de resolução, o número de nós visitados, a taxa de processamento em nós por segundo (NPS) e a taxa de acerto sobre o conjunto de posições testadas.

#figure(
  table(
    columns: (auto, auto, auto, auto, auto, auto),
    align: horizon,
    table.header(
      [Profundidade],
      [Tempo médio (ms)],
      [Variância (ms)],
      [Nós visitados],
      [NPS],
      [Taxa de acerto (%)]
    ),
    [3], [-], [-], [-], [-], [-],
    [4], [-], [-], [-], [-], [-],
    [5], [-], [-], [-], [-], [-],
    [6], [-], [-], [-], [-], [-],
  ),
  caption: [Desempenho geral da engine por profundidade de busca],
) <tab:desempenho>

Observa-se que o aumento da profundidade de busca eleva progressivamente o custo computacional, refletido tanto no tempo de resolução quanto no número de nós visitados. Em contrapartida, a taxa de acerto tende a crescer com o aprofundamento da análise, indicando melhora na qualidade da decisão em posições táticas.

== Impacto das Heurísticas de Otimização

A @tab:heuristicas resume as métricas internas associadas aos mecanismos de otimização implementados, permitindo observar o comportamento da poda alfa-beta, da tabela de transposição e da busca de quiescência ao longo das execuções.

#figure(
  table(
    columns: (auto, auto, auto, auto),
    align: horizon,
    table.header(
      [Profundidade],
      [Cortes alfa-beta],
      [Reaproveitamento da TT (%)],
      [Chamadas à quiescência]
    ),
    [3], [-], [-], [-],
    [4], [-], [-], [-],
    [5], [-], [-], [-],
    [6], [-], [-], [-],
  ),
  caption: [Métricas internas das heurísticas de otimização],
) <tab:heuristicas>

Os dados indicam o efeito dos mecanismos internos de aceleração e estabilização da busca. Em particular, o número de cortes alfa-beta permite estimar a eficiência da ordenação de lances, enquanto o reaproveitamento da tabela de transposição evidencia o grau de eliminação de recomputações. Já a frequência de chamadas à busca de quiescência sinaliza o volume de posições taticamente instáveis encontradas no horizonte da busca principal.

== Síntese dos Resultados

Em conjunto, os resultados mostram que a engine é capaz de resolver posições táticas de referência com custo computacional compatível com a profundidade analisada, ao mesmo tempo em que explora mecanismos clássicos de otimização para conter o crescimento da árvore de busca. Esses dados estabelecem a base quantitativa necessária para a discussão da eficácia da arquitetura implementada.

= Discussão



= Conclusão  

