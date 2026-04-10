#import "../lib.typ": ieee

#let titulo = "Desenvolvimento de um Gerador de Lances para Xadrez em Rust com Bitboards e Magic Bitboards"
#let authors = ("Guilherme Borges Pagano", "João Marcos de Oliveira Calixto")
#let data = "16 de junho de 2025"

#show: ieee.with(
    title: titulo,
    abstract: [
        A aplicação de inteligência artificial ao xadrez depende de estruturas de dados e algoritmos capazes de representar o tabuleiro e gerar lances com baixo custo computacional. Este artigo apresenta a ChessLib, uma biblioteca de xadrez implementada em Rust, voltada à representação eficiente do estado do jogo e à geração de lances baseada em bitboards e _magic bitboards_. A solução explora operações bitwise, tabelas de ataque pré-computadas e uma organização modular orientada à extensibilidade, com ênfase em desempenho e segurança de memória. Além de descrever a arquitetura adotada, o trabalho estabelece um protocolo de avaliação experimental baseado em testes _Perft_ e em comparação com bibliotecas de referência do domínio.
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
    index-terms: ("Rust", "Xadrez", "Inteligência Artificial", "Bitboards", "Geração de lances;"),
    bibliography: bibliography("refs.yml", title: "Referências", style: "ieee"),
)

= Introdução

O xadrez ocupa, há décadas, um papel de destaque na pesquisa em inteligência artificial, servindo como ambiente de teste para técnicas de busca, avaliação heurística e, mais recentemente, aprendizado por reforço. A relevância desse domínio decorre do fato de que o desempenho de um sistema enxadrístico depende tanto da qualidade de sua estratégia de decisão quanto da eficiência com que o estado do jogo é representado e manipulado.

O xadrez ocupa, há décadas, um papel central na pesquisa em inteligência artificial, servindo como ambiente de teste para técnicas de busca, avaliação heurística e, mais recentemente, aprendizado por reforço @campbell2002deepblue. Nesse domínio, o desempenho do sistema depende não apenas da qualidade da estratégia de decisão, mas também da eficiência com que o estado do jogo é representado e manipulado.

Marcos históricos como o Deep Blue evidenciaram a força de abordagens baseadas em busca altamente otimizada e conhecimento especializado de domínio @campbell2002deepblue. Em seguida, sistemas como o AlphaZero e projetos abertos como o Leela Chess Zero reforçaram a relevância de arquiteturas apoiadas em autojogo e redes neurais profundas @silver2017alphazero @lc0overview. Apesar dessas diferenças na camada de decisão, todos esses sistemas dependem de uma infraestrutura de geração de lances correta e eficiente.

Nesse contexto, este trabalho apresenta o desenvolvimento da ChessLib, uma biblioteca de xadrez implementada em Rust, com foco em eficiência, segurança de memória e organização modular. A biblioteca adota bitboards como estrutura principal de representação do tabuleiro e emprega magic bitboards para otimizar a geração de lances de peças deslizantes, explorando operações bitwise e acesso pré-computado a tabelas de ataque @bitboards @kannan2007magic.

A proposta insere-se no contexto de bibliotecas de base para _engines_ de xadrez e ferramentas correlatas, priorizando uma infraestrutura reutilizável para futuras extensões, como mecanismos de busca, funções de avaliação e integração com agentes de inteligência artificial. Assim, a contribuição principal deste artigo está na descrição da arquitetura da ChessLib, das decisões de implementação adotadas e da metodologia experimental proposta para avaliar sua corretude funcional e seu desempenho computacional.

= Trabalhos Relacionados

O desenvolvimento de software para xadrez pode ser analisado, de forma geral, em duas frentes complementares. A primeira corresponde aos _engines_ completos, concebidos para selecionar lances e disputar partidas de forma autônoma. A segunda reúne bibliotecas de lógica de xadrez, voltadas à representação do estado do jogo, à aplicação de regras e à geração de lances. 

== Engines de Xadrez de Alta Performance

Entre as _engines_ open-source contemporâneas, o Stockfish destaca-se como uma das principais referências de desempenho, combinando busca baseada em poda alfa-beta com heurísticas avançadas e avaliação por redes neurais eficientes no formato NNUE @stockfishdocs. Em paralelo, o Leela Chess Zero (Lc0) representa a abordagem baseada em redes neurais profundas e autojogo, inspirada pela linha introduzida pelo AlphaZero @silver2017alphazero @lc0overview. Esses dois projetos ilustram paradigmas centrais da computação enxadrística atual e reforçam a importância da geração eficiente de lances como componente estrutural de sistemas competitivos. Além disso, a exploração de linguagens modernas para o desenvolvimento de _engines_ de alto desempenho não se restringe a implementações em C++ nem a fluxos de experimentação apoiados em Python: trabalhos recentes também investigam a linguagem Go como base para arquiteturas enxadrísticas modulares e competitivas, como exemplifica a _engine_ GoFish @gofish2024.

== Bibliotecas de Lógica de Xadrez

Além dos _engines_ completos, há bibliotecas especializadas na modelagem do jogo e na manipulação programática do tabuleiro. Essas bibliotecas são particularmente úteis em cenários de prototipagem, ensino, experimentação algorítmica e integração com aplicações maiores, como analisadores, interfaces gráficas, ferramentas de teste e sistemas de inteligência artificial.

No ecossistema Python, a biblioteca "python-chess" tornou-se uma referência amplamente adotada por oferecer representação de posições, geração de lances, validação de legalidade e manipulação de formatos usuais do domínio enxadrístico @pythonchess. No entanto, por estar inserida em um ambiente interpretado, sua utilização em cenários de alta intensidade computacional tende a apresentar limitações de desempenho quando comparada a implementações em linguagens compiladas.

No ecossistema Rust, a biblioteca "chess" oferece uma referência importante de implementação eficiente para representação do tabuleiro e geração de lances, demonstrando a viabilidade de soluções de alto desempenho nesse ambiente @bray2024chess.

Nesse contexto, Rust oferece características particularmente relevantes para bibliotecas centrais de xadrez, como desempenho próximo ao de linguagens de sistema, controle explícito de memória e ausência de coletor de lixo, com garantias estáticas de segurança documentadas tanto em sua documentação oficial quanto em literatura acadêmica da ACM sobre a linguagem e seu uso em sistemas @rustbook @matsakis2014rustsafe.

A ChessLib insere-se nesse cenário como uma biblioteca de xadrez em Rust voltada à construção de uma base modular e eficiente para representação do jogo e geração de lances. Sua proposta não é competir diretamente com _engines_ completos, mas oferecer uma infraestrutura reutilizável, com ênfase em corretude funcional, desempenho e extensibilidade.

= Arquitetura

A ChessLib foi projetada como uma biblioteca modular para representação de posições e geração eficiente de lances em Rust. Sua implementação inspira-se em bibliotecas enxadrísticas de alto desempenho, com ênfase em estrutura compacta de dados, baixo custo de acesso à memória e separação clara entre representação do estado, pré-cálculo de ataques e geração de movimentos @bray2024chess.

== Representação do Tabuleiro

A biblioteca adota bitboards como estrutura principal de representação. Nessa abordagem, o tabuleiro é modelado por inteiros de 64 bits, nos quais cada bit corresponde a uma casa. Em vez de estruturas bidimensionais tradicionais, essa modelagem permite representar conjuntos de casas de forma compacta e manipulá-los por meio de operações bitwise executadas diretamente pela CPU @bitboards.

Na convenção utilizada, baseada em _Little-Endian File Mapping_, a casa "a1" corresponde ao bit menos significativo e "h8" ao mais significativo. A partir dessa organização, a posição pode ser descrita por múltiplos bitboards, tipicamente separados por tipo de peça e cor, além de estruturas agregadas para ocupação total, peças brancas e peças pretas. Essa organização, evidenciada na @Fig1, simplifica consultas de ocupação, detecção de ataques e aplicação de máscaras sobre regiões específicas do tabuleiro.

#figure(
  image("./assets/grid.png", width: 70%),
  caption: [Little-Endian File Mapping],
)<Fig1>

== Geração de Lances

A geração de lances na ChessLib é dividida entre peças de passo e peças deslizantes. Em ambos os casos, a lógica procura deslocar o máximo possível do custo computacional para tabelas pré-calculadas e operações bitwise simples em tempo de execução.

=== Peças de Passo

Para cavalo e rei, os padrões de ataque dependem apenas da casa de origem; por isso, seus movimentos podem ser pré-computados para as 64 casas e depois filtrados com base na ocupação por peças da mesma cor. 

Um cuidado importante nessa etapa é evitar o problema de _wrap-around_, no qual deslocamentos de bits podem produzir ataques inválidos entre bordas opostas do tabuleiro. Esse efeito é evitado mediante máscaras de arquivo aplicadas antes ou depois dos deslocamentos, conforme o padrão de movimento.

No caso dos peões, a geração é mais particular, pois envolve avanço simples, avanço duplo, capturas diagonais, promoção e _en passant_. 

O avanço simples pode ser modelado por deslocamento vertical e filtragem pelas casas vazias; o avanço duplo exige, adicionalmente, que a peça esteja na fileira inicial e que não haja bloqueio intermediário. As capturas diagonais também são expressas com deslocamentos bitwise, combinados com máscaras para impedir _wrap-around_. Já promoções e _en passant_ dependem de informação adicional de estado, exigindo tratamento específico na lógica de aplicação e validação dos lances.

=== Peças Deslizantes

Para torres, bispos e damas, entretanto, a geração de lances é substancialmente mais complexa. O conjunto de ataques dessas peças depende da configuração dos bloqueadores presentes ao longo de raios horizontais, verticais ou diagonais. Uma abordagem ingênua, baseada em varrer cada direção em tempo de execução para cada peça, produz custo elevado. Por isso, a ChessLib adota a técnica de _magic bitboards_, que substitui esse processo por indexação em tabelas pré-calculadas @kannan2007magic @magicbitboards.

== Magic Bitboards

O objetivo central dos _magic bitboards_ é transformar o problema da geração de ataques de peças deslizantes em um problema de consulta eficiente. Para uma peça deslizante em uma determinada casa, os lances possíveis dependem apenas dos bloqueadores relevantes presentes nos seus raios de ação. Em vez de recalcular esses ataques dinamicamente a cada consulta, a técnica associa cada configuração relevante de bloqueadores a um índice compacto em uma tabela de ataques pré-computada.

A indexação é feita por uma função de hash baseada em multiplicação e deslocamento:

*#raw(
  "index = ((occupancy & mask) * magic) >> shift",
  lang: "python",
)*

Nessa expressão, "occupancy" representa a ocupação atual do tabuleiro, "mask" seleciona apenas os bloqueadores relevantes para a peça e a casa em questão, "magic" é uma constante de 64 bits escolhida especificamente para essa configuração, e "shift" reduz o resultado a um índice compacto. 

O valor produzido é então utilizado para acessar uma tabela cujo conteúdo é um bitboard contendo os ataques válidos correspondentes.

A implementação depende de três elementos principais:

=== Máscara de bloqueadores 

Esse elemento define quais casas realmente influenciam os ataques de uma peça deslizante em determinada casa. Essas casas correspondem aos raios de movimento da peça, excluindo a própria casa de origem, bem como as casas de borda. Essa exclusão é uma otimização importante, pois o estado da casa terminal de um raio é redundante para distinguir conjuntos de ataque, permitindo reduzir o número de combinações relevantes e, consequentemente, o tamanho das tabelas.

=== Número Mágico

O número mágico é uma constante de 64 bits, única para cada casa e tipo de peça (torre/bispo), que foi descoberta através de uma busca por força bruta para satisfazer a propriedade de hashing perfeito para a máscara de bloqueadores dessa casa. 

Estes números não são derivados de uma fórmula matemática, mas são encontrados através de um processo de tentativa e erro. Embora esse procedimento seja empírico, a técnica possui validação e lastro na literatura científica de jogos de tabuleiro, tendo sido formalmente proposta e implementada também no contexto do Shogi @yamamoto2010shogi. A comunidade de programação de xadrez mantém listas dos "melhores mágicos até agora", que são números que não só funcionam, mas também permitem tabelas de ataque mais compactas @fiekas2018magic. 

Cada uma das 128 combinações (64 para torres, 64 para bispos) tem o seu próprio número mágico único.

=== Tabela de Ataques

Por fim, a tabela de ataques armazena, para cada índice válido, o bitboard correspondente ao conjunto de movimentos possíveis. Depois de inicializada, a consulta em tempo de execução torna-se extremamente barata: basta isolar os bloqueadores relevantes, calcular o índice mágico e recuperar o bitboard de ataques armazenado.

A etapa de inicialização consiste justamente em construir essas tabelas e validar números mágicos adequados para bispos e torres em cada uma das 64 casas. Embora essa fase seja relativamente trabalhosa, ela é executada apenas uma vez, deslocando o custo computacional para fora do caminho crítico da geração de lances. O processo de busca de um número mágico pode ser resumido pelo fluxo apresentado na @lofa.

#figure(
  image("./assets/lofa.drawio.svg", width: 84%),
  caption: "Geração e validação de um número mágico",
)<lofa>

Uma vez inicializado, o processo de geração de movimentos em tempo de execução é extremamente eficiente, consistindo em uma sequência linear de operações apresentadas na @talofa.

#figure(
  image("./assets/talofa.drawio.svg", width: 35%),
  caption: "Consulta de ataques em tempo de execução com magic bitboards",
)<talofa>

Essa estratégia permite combinar pré-computação, compacidade e eficiência, tornando os magic bitboards a referência consolidada para geração de lances de peças deslizantes em programas de xadrez baseados em bitboards @kannan2007magic.

= Avaliação Experimental

A avaliação experimental foi estruturada para analisar a ChessLib em duas dimensões complementares: 

    +   Corretude funcional da geração de lances; 
    +   Desempenho computacional da biblioteca em cenários representativos. 

Para isso, adotou-se o teste _Perft_ (_performance test_), amplamente utilizado em computação enxadrística como procedimento de validação da geração de lances e como base para comparação de desempenho entre implementações @perft.

== Objetivos da Avaliação

O primeiro objetivo é verificar se a ChessLib gera exatamente o conjunto de lances esperado para diferentes posições e profundidades. O segundo é medir o custo computacional da geração de lances em uma carga padronizada. 

Embora o _Perft_ não substitua a avaliação de uma _engine_ completa, ele constitui um critério adequado para comparar bibliotecas cuja principal responsabilidade é representar o estado do jogo e enumerar movimentos de forma eficiente.

== Ambiente Experimental

A @system documenta o hardware e o sistema operacional utilizados nos experimentos.

#figure(
  table(
    columns: (auto, 1fr),
    align: horizon,
    table.header([Componente], [Especificação]),
    [CPU],                 [Intel Core i7-12800H @ 4.70GHz],
    [Núcleos/Threads],     [14 Cores / 20 Threads],
    [RAM],                 [32 GB],
    [Sistema Operacional], [Arch Linux x86_64 6.12.42-1-lts],
  ),
  caption: [Ambiente computacional utilizado nos experimentos],
) <system>

A @libs registra as versões das bibliotecas comparadas, uma vez que alterações de implementação entre versões podem impactar diretamente o desempenho.

#figure(
  table(
    columns: (1fr, auto),
    align: horizon,
    table.header([Biblioteca], [Versão]),
    [ChessLib],             [-],
    [chess],                [-],
    [shakmaty],             [-],
    [Stockfish via UCI],    [-],
    [python-chess],         [-],
  ),
  caption: [Implementações avaliadas nos experimentos],
) <libs>

A ChessLib foi comparada com quatro implementações de referência:

    -   "python-chess", uma biblioteca de xadrez consolidada em Python @pythonchess;
    -   "chess", uma crate em Rust orientada a alto desempenho @bray2024chess;
    -   "shakmaty", uma biblioteca geral de xadrez em Rust @shakmatyreadme;
    -   "Stockfish", avaliado por meio de chamadas externas via protocolo UCI @stockfishdocs.

Desse modo, a avaliação busca situar a ChessLib tanto em relação a bibliotecas embutidas no mesmo processo quanto, de forma complementar, em relação ao custo prático de integração com uma _engine_ externa.

== Conjunto de Posições de Teste

Os experimentos foram definidos a partir de um conjunto de posições em notação Forsyth-Edwards (FEN), formato amplamente empregado para representar estados completos de uma partida de xadrez @pgnspec. A seleção das posições teve como objetivo variar a carga de processamento e cobrir aspectos específicos da lógica de geração de lances.

=== Posição Inicial 

A posição inicial foi adotada como referência básica por possuir resultados canônicos para diferentes profundidades de _Perft_ @perftresults.

`rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1` 

=== Teste de capturas

Uma posição clássica de _perft_ com alta incidência de capturas e tática imediata, útil para estressar verificações de legalidade e a enumeração de respostas forçadas.

`rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8`

=== "Kiwipete"

Uma posição complexa de meio-jogo com muitas possibilidades táticas, incluindo roques, capturas e lances de peão. Testa o desempenho num cenário mais realista e computacionalmente denso.

`r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1`

=== Teste de promoção

Uma posição projetada especificamente para testar a lógica de promoção de peões, que pode ser uma fonte de bugs e ineficiências.

`n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1`

Na posição inicial, os benchmarks principais foram executados nas profundidades 4 a 7. Nos presets específicos, foram avaliadas as posições `captures` na profundidade 5, `promotions` na profundidade 6 e `kiwipete` na profundidade 5. As implementações `chesslib-simple` e `python-chess` foram incluídas apenas na rodada de profundidade 4 da posição inicial, em caráter complementar.

= Resultados

== Observações Metodológicas

A execução do Stockfish ocorreu por meio de um processo externo acionado via UCI, de modo que seus tempos incluem inicialização do processo, _handshake_ do protocolo e comunicação por _stdin/stdout_; assim, eles não são diretamente comparáveis ao custo _in-process_ das crates Rust.

ChessLib-Simple é uma implementação alternativa da ChessLib, que não utiliza, propositalmente, a técnica de bitboards.

ChessLib-Simple e Python-Chess só foram incluídos na rodada da profundidade 4 da posição inicial.

== Posição Inicial


#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [1.2],
    [ChesLib],           [1.5],
    [Shakmaty],          [1.6],
    [Stockfish via UCI], [161.4],
    [ChessLib-Simple],   [23.3],
    [Python-Chess],      [235.0],
  ),
  caption: [Benchmark na posição inicial para $d=4$],
) <benchmark1>

==== NOTA
Esta rodada tem baixa confiabilidade para os binários de Rust por estar abaixo de 5 ms.


#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [11.9],
    [ChesLib],           [14.4],
    [Shakmaty],          [15.8],
    [Stockfish via UCI], [173.8],
  ),
  caption: [Benchmark na posição inicial para $d=5$],
) <benchmark2>


#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [241.9],
    [ChesLib],           [259.5],
    [Shakmaty],          [350.2],
    [Stockfish via UCI], [500.9],
  ),
  caption: [Benchmark na posição inicial para $d=6$],
) <benchmark3>


#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (s)], 
    ),
    [Chess],             [6.251],
    [ChesLib],           [7.467],
    [Shakmaty],          [9.318],
    [Stockfish via UCI], [9.974],
  ),
  caption: [Benchmark na posição inicial para $d=7$],
) <benchmark4>

== Posição de capturas ($d=5$)

#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [128.1],
    [ChesLib],           [142.5],
    [Shakmaty],          [249.0],
    [Stockfish via UCI], [445.1],
  ),
  caption: [Benchmark na posição de capturas para $d=5$],
) <benchmark5>

== Posição de promoções ($d=6$)

#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [177.2],
    [ChesLib],           [195.4],
    [Shakmaty],          [298.0],
    [Stockfish via UCI], [570.0],
  ),
  caption: [Benchmark na posição de promoções para $d=6$],
) <benchmark6>

== Posição "Kiwipete" ($d=5$)

#figure(
  table(
    columns: (1fr, 1fr),
    align: horizon,
    table.header(
        [Engine], 
        [Tempo médio (ms)], 
    ),
    [Chess],             [261.5],
    [ChesLib],           [282.9],
    [Shakmaty],          [537.3],
    [Stockfish via UCI], [731.7],
  ),
  caption: [Benchmark na posição "Kiwipete" para $d=5$],
) <benchmark7>

Para complementar a análise, a @medias resume os tempos médios obtidos nos presets "captures", "promotions" e "kiwipete".

#figure(
  table(
    columns: (1.2fr, 1fr, 1fr, 1fr, 1fr, 1fr),
    align: horizon,
    table.header(
        [Posição de teste], 
        [Profundidade], 
        [Chess], 
        [ChessLib], 
        [Shakmaty], 
        [Stockfish via UCI]
    ),
    [Captures],   [5], [128.1 ms], [142.5 ms], [249.0 ms], [445.1 ms],
    [Promotions], [6], [177.2 ms], [195.4 ms], [298.0 ms], [570.0 ms],
    [Kiwipete],   [5], [261.5 ms], [282.9 ms], [537.3 ms], [731.7 ms],
  ),
  caption: [Tempos médios em posições específicas de teste],
) <medias>

= Discussão

Nos cenários avaliados, a crate "chess" apresentou o melhor desempenho bruto, enquanto a ChessLib permaneceu consistentemente em segundo lugar, à frente de "shakmaty". Mais do que a diferença para a "chess", porém, o contraste com a variante "chesslib-simple" ajuda a evidenciar o peso da representação interna adotada. Sem o mesmo aproveitamento de _bitboards_ no caminho crítico, ela permaneceu em um patamar próximo ao de "python-chess".

Além disso, os resultados são relevantes porque a ChessLib não se limita à geração mínima de lances: ela também mantém informações incrementais de estado, como _Zobrist hash_, contagem de _half-moves_ e compatibilidade com fluxos associados ao protocolo UCI. Assim, parte do custo medido decorre de responsabilidades adicionais no caminho crítico de execução, sem eliminar o ganho estrutural proporcionado pela representação escolhida.

Já a comparação com o Stockfish deve ser interpretada com cautela. No arranjo experimental utilizado, os tempos medem o custo de acionar uma _engine_ externa via _shell_ e UCI, e não apenas sua rotina interna de _perft_. Ainda assim, sua inclusão é útil para reforçar que sistemas completos de xadrez assumem responsabilidades adicionais além da simples enumeração de lances.

= Conclusão  

Este artigo apresentou a ChessLib, uma biblioteca de xadrez em Rust voltada à representação eficiente do tabuleiro e à geração de lances com base em bitboards e _magic bitboards_.

No plano experimental, os resultados mostraram que a crate "chess" obteve o melhor desempenho bruto nos cenários avaliados. Ainda assim, a ChessLib manteve-se consistentemente como a segunda implementação mais rápida entre as alternativas comparadas no mesmo processo, frequentemente com diferença moderada em relação à líder e com vantagem clara sobre abordagens mais gerais. Esse resultado é significativo porque a proposta da ChessLib não se restringe ao núcleo mínimo de geração de lances, incluindo também a manutenção incremental de informações de estado relevantes para uso prático da biblioteca.

Desse modo, o trabalho não demonstra a superação da principal referência de desempenho em Rust, mas mostra que é possível alcançar uma implementação competitiva e tecnicamente robusta mesmo conciliando geração eficiente de lances com responsabilidades adicionais de estado e integração. Como desdobramento, a biblioteca pode ser estendida com a implementação de uma inteligência artificial que utilize a infraestrutura de geração de lances para avaliar posições e selecionar os melhores movimentos, o que constitui um passo natural para explorar o potencial da ChessLib em cenários de jogo autônomo.