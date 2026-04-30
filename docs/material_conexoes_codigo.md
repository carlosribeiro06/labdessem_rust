# Mapa De Conexoes Do Codigo Do LabDessem Rust

## 1. Objetivo

Este material foi escrito para responder a uma pergunta muito prática:

> "como cada coisa no código se conecta com o restante do projeto?"

A ideia aqui não é repetir o manual de desenvolvimento focado em implementação. Este documento é um **mapa de conexões**:

- quem chama quem;
- onde os dados entram;
- onde os dados viram estruturas de domínio;
- onde essas estruturas viram variáveis e restrições;
- onde o solver entra;
- onde os resultados voltam para o sistema;
- onde os relatórios CSV são gerados.

Em outras palavras: este arquivo foi feito para você abrir e entender o **encadeamento real do projeto**.

---

## 2. Visão global: o ciclo completo

O ciclo do programa hoje é:

1. O executável lê a configuração do estudo.
2. A camada de IO lê o caso inteiro e monta um `System`.
3. A camada de modelo transforma o `System` em um `Model`.
4. A camada de solver resolve esse `Model`.
5. A camada de simulação decide quantas vezes resolver, em que modo e com quais ajustes.
6. A camada de simulação também interpreta a solução e escreve os arquivos de saída.

Fluxo resumido:

```text
study_config.json
    ↓
labdessem-cli
    ↓
labdessem-io::read_study_from_config
    ↓
System
    ↓
labdessem-model::Model::from_system
    ↓
Indexing + Variables + ConstraintSet + Objective
    ↓
labdessem-solver::solve_model
    ↓
SolveSummary
    ↓
labdessem-simulation
    ↓
RESULTADOS/*.csv
```

---

## 3. Ponto de entrada: `labdessem-cli`

Arquivo principal:

- [main.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-cli/src/main.rs)

### O que acontece aqui

O CLI faz três coisas essenciais:

1. descobre o caminho do `study_config.json`;
2. chama a leitura do estudo;
3. chama a simulação e grava resultados.

### Encadeamento real

No `main.rs`, o fluxo é:

1. `read_study_config(config_path)`
2. `ExecutionOption::from_config_value(...)`
3. `read_study_from_config(config_path)`
4. `run_simulation(&system, network_enabled, execution_option)`
5. `write_results_csvs(&system, &result, &output_dir, network_enabled)`

Então, a conexão principal do CLI é:

```text
CLI → IO → System → Simulation → Output CSV
```

O CLI não formula restrição, não cria variável, não resolve diretamente o solver. Ele apenas orquestra o início e o fim do processo.

---

## 4. A fronteira entre arquivo e modelo: `labdessem-io`

Arquivo central:

- [study.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/src/study.rs)

Essa é a camada que converte arquivos do estudo em objetos do domínio.

### Funções-chave

#### `read_study_config`

Lê o `study_config.json` e resolve o `case_path`.

Ela produz um `StudyConfig`, não um `System`.

#### `read_study_from_config`

Lê o `StudyConfig` e chama:

```text
read_study_from_path_with_options(...)
```

Essa função é a porta de entrada real para a construção do caso.

#### `read_study_from_path_with_options`

Essa é uma das funções mais importantes do projeto.

Ela:

- define pastas `CAD/` e `OPER/`;
- lê todos os arquivos relevantes;
- chama os builders internos;
- monta o `System`;
- valida o `System`.

### Builders importantes

Dentro de `study.rs`, você encontra uma família de funções `build_*`. Elas formam a ponte entre os CSVs e o domínio:

- `build_horizon`
- `build_submarkets`
- `build_buses`
- `build_branches`
- `build_interchange_limits`
- `build_residual_costs`
- `build_thermal_plants`
- `build_hydro_plants`
- `build_pumping_plants`
- `build_renewables`
- `build_operational_limits`

### Conexão conceitual

Cada builder recebe linhas lidas do CSV e devolve objetos do `labdessem-core`.

Exemplo:

```text
CAD_UHE.csv + CAD_CONJ_UHE.csv + OPER_VAZAO.csv + OPER_FPHA.csv
    ↓
build_hydro_plants(...)
    ↓
Vec<HydroPlant>
```

Outro exemplo:

```text
CAD_REN.csv + OPER_REN.csv
    ↓
build_renewables(...)
    ↓
Vec<RenewablePlant>
```

### O que nasce aqui e vai influenciar tudo depois

Tudo que é estrutural nasce nessa camada:

- horizonte;
- demanda;
- rede elétrica;
- hidrelétricas;
- térmicas;
- renováveis;
- elevatórias;
- limites;
- custos;
- flags de UC.

Se um dado não entrou corretamente em `System`, o resto do pipeline inteiro já nasce comprometido.

---

## 5. O núcleo semântico do projeto: `labdessem-core`

Arquivos principais:

- [system.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/system.rs)
- [thermal.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/thermal.rs)
- [hydro.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/hydro.rs)
- [renewable.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/renewable.rs)
- [ids.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-core/src/ids.rs)

### Papel do `core`

O `core` é onde o projeto decide:

- o que existe no mundo do problema;
- quais campos cada entidade possui;
- quais validações estruturais são obrigatórias.

Ele não sabe ler CSV.
Ele não sabe resolver LP.
Ele não sabe escrever CSV de saída.

Mas ele define **o vocabulário do problema**.

### `System`

O `System` é o objeto central do caso.

Hoje ele conecta:

- `StudyHorizon`
- `Submarket`
- `Bus`
- `Branch`
- `ThermalPlant`
- `HydroPlant`
- `PumpingPlant`
- `RenewablePlant`
- `OperationalLimit`
- `InterchangeLimit`
- `ResidualCost`

### Conexão estrutural

`study.rs` monta o `System`.

Depois:

```text
System
    ↓
Indexing::from_system
Variables::for_system
ConstraintSet::for_system
Objective::for_system
```

Ou seja, o `System` é a fonte de verdade para a formulação matemática.

### Papel das validações

O `System::validate()` chama validações internas e checa coerência global:

- ids únicos;
- submercados conhecidos;
- barras conhecidas;
- consistência entre bus e submercado;
- cascata hidráulica;
- limites físicos;
- tamanhos de séries temporais.

Isso significa:

- `io` traduz arquivo em estrutura;
- `core` decide se essa estrutura faz sentido.

---

## 6. O primeiro passo da formulação: `Indexing`

Arquivo:

- [indexing.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/indexing.rs)

### Por que ele existe

O `System` é hierárquico:

- usina térmica → unidades
- usina hidráulica → conjuntos → unidades
- submercados
- períodos

O solver, por outro lado, prefere vetores planos.

O `Indexing` existe para fazer essa ponte.

### O que ele produz

Ele cria listas como:

- `thermal_unit_entries`
- `hydro_unit_entries`
- `hydro_plant_entries`
- `renewable_plant_entries`
- `pumping_plant_entries`
- `interchange_entries`

Cada entrada guarda:

- qual planta é;
- qual unidade é;
- qual grupo é;
- qual submercado é.

### Conexão direta

`Variables::for_system` usa o `Indexing` para criar vetores de variáveis.

`ConstraintSet::for_system` usa o `Indexing` para localizar quais variáveis pertencem a cada entidade.

Então o `Indexing` é a camada que responde:

> “qual posição do vetor corresponde a esta usina/unidade/submercado?”

---

## 7. A criação das variáveis: `Variables`

Arquivo:

- [variables.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/variables.rs)

### Função-chave

- `Variables::for_system(system, indexing, solve_mode)`

Essa função lê o `System` e cria todas as famílias de variáveis do modelo.

### Conexão de dados

Exemplo mental:

```text
HydroPlant / HydroUnit / Horizon
    ↓
hydro_generation
hydro_turbining
hydro_volume
hydro_spillage
```

Outro exemplo:

```text
RenewablePlant / Horizon
    ↓
renewable_generation
```

Outro:

```text
ThermalPlant / ThermalUnit / Horizon
    ↓
thermal_generation
thermal_commitment
thermal_startup
thermal_shutdown
```

### O que cada variável já carrega

Cada `Variable` sai dessa etapa com:

- nome;
- limite inferior;
- limite superior;
- domínio contínuo/binário;
- valor fixo, se existir.

Então a conexão é:

```text
System + Indexing
    ↓
Variables
    ↓
solver variables
```

### Importante

Nesta etapa as variáveis ainda não têm significado econômico ou físico completo.

Elas só passam a “fazer algo” quando entram:

- na função objetivo;
- nas restrições.

---

## 8. A montagem das restrições: `ConstraintSet`

Arquivo:

- [constraints.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/constraints.rs)

### Função-chave

- `ConstraintSet::for_system(system, indexing, variables, solve_mode)`

Ela agrega todas as famílias de restrição do modelo.

### Ordem de construção

O arquivo monta, em sequência:

- balanço de demanda;
- balanço hídrico;
- limites de intercâmbio;
- limite de turbinamento;
- não negatividade de vertimento;
- FPHA;
- acoplamento geração-turbinamento;
- limites operativos;
- restrições de UC térmico, se aplicável;
- restrições de rampa térmica, se aplicável;
- restrições de UC hidráulico, se aplicável.

### Conexão central

Cada builder de restrição usa:

- `system` para parâmetros;
- `indexing` para localizar as entidades;
- `variables` para referenciar as variáveis corretas.

Exemplo conceitual:

```text
system.hydro_plants
+ indexing.hydro_unit_entries
+ variables.hydro_turbining
    ↓
build_hydro_balance_constraints(...)
```

### O papel dos nomes das restrições

Toda restrição recebe nome. Isso conecta várias partes do projeto:

- testes localizam restrições pelo nome;
- `simulation` procura duais pelo nome;
- relatórios usam nomes para recuperar significados;
- debug fica muito mais simples.

Exemplo:

```text
hydro_balance[p=CAMARGOS,t=1]
demand_balance[submarket=SE,t=1]
hydro_fpha[p=CAMARGOS,seg=3,t=12]
```

Então o nome de restrição não é cosmético. Ele é uma interface interna do projeto.

---

## 9. A função objetivo: `Objective`

Arquivo:

- [objective.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/objective.rs)

### Função-chave

- `Objective::for_system(system, indexing, variables, solve_mode)`

### Conexão conceitual

A função objetivo conecta:

- custos do domínio (`System`);
- variáveis do modelo (`Variables`);
- modo de resolução (`SolveMode`).

Exemplos de termos:

- CVU térmico
- custo de déficit
- penalidade de vertimento
- penalidade de turbinamento
- custo de partida/desligamento
- custo residual de TON
- penalidade de intercâmbio

### Ligação com as unidades

Essa é uma camada onde a consistência dimensional é crítica.

Exemplo:

```text
thermal_generation em MW
CVU em R$/MWh
→ multiplicar por duração do período
```

Já:

```text
hydro_turbining em hm3 por período
penalidade em R$/hm3
→ não multiplicar por duração
```

Então a conexão aqui não é só estrutural. É também uma conexão entre:

- conceito físico;
- unidade matemática;
- custo econômico.

---

## 10. O empacotamento do modelo: `Model::from_system`

Arquivo:

- [labdessem-model/src/lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-model/src/lib.rs)

### Papel

Essa função junta tudo:

```text
System
    ↓
Indexing
    ↓
Variables
    ↓
ConstraintSet
    ↓
Objective
    ↓
Model
```

Ela é uma função charneira do projeto.

Quando a simulação quer resolver o problema, ela quase sempre passa por:

```text
Model::from_system(...)
```

---

## 11. A conversão para solver: `labdessem-solver`

Arquivo:

- [labdessem-solver/src/lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-solver/src/lib.rs)

### Função-chave

- `solve_model(model)`

### O que acontece aqui

1. `collect_variables(model)` junta todas as famílias de variáveis.
2. O código cria as variáveis do `good_lp`.
3. A função objetivo é convertida em `Expression`.
4. Cada `LinearConstraint` é convertida em restrição do solver.
5. O solver resolve o problema.
6. O resultado volta como `SolveSummary`.

### `SolveSummary`

É a ponte entre solver e simulação.

Ele contém:

- `objective_value`
- `variable_values`
- `constraint_duals`

Ou seja:

```text
Model
    ↓
good_lp
    ↓
SolveSummary
```

### Conexão importante

Os nomes das variáveis e restrições criados no modelo são usados diretamente como chaves do `HashMap` de saída.

Então:

- se o nome da variável muda, os relatórios precisam saber disso;
- se o nome da restrição muda, a recuperação de dual precisa saber disso.

---

## 12. A orquestração da resolução: `labdessem-simulation`

Arquivo:

- [labdessem-simulation/src/lib.rs](C:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-simulation/src/lib.rs)

Essa é a camada mais conectada do projeto.

### Por quê

Porque ela conversa com:

- `System`
- `Model`
- `Solver`
- rede elétrica
- iterações
- saídas CSV

### Funções centrais

#### `run_simulation`

Decide a estratégia de execução:

- iterativa;
- MILP único.

#### `run_iterative_simulation_strategy`

Roda o processo:

- LP
- MILP
- LP-FIXED
- LP-CALC-CMO

e gerencia cortes de rede.

#### `run_single_milp_simulation`

Resolve em etapa única quando `opcao_execucao = 2`.

#### `solve_stage`

Essa função é a ponte mais direta entre simulação e formulação.

Ela:

1. monta `Model::from_system(...)`;
2. injeta ajustes específicos da etapa;
3. chama `solve_model(...)`;
4. coleta violações e relatórios.

Então a conexão central da simulação é:

```text
System
    ↓
Model::from_system
    ↓
solve_model
    ↓
SolveSummary
    ↓
regras iterativas e saídas
```

### Rede elétrica

A simulação também constrói:

- `DcNetworkModel::from_system(system)`

Esse modelo de rede calcula:

- conjunto de barras sem slack;
- PTDF;
- fluxos por linha.

Ele é usado para:

- detectar violações;
- adicionar cortes;
- montar arquivos de resultado da rede.

### Inviabilidades

Além da solução principal, essa camada agrega:

- inviabilidades de rede;
- inviabilidades de limite operativo.

Essas informações vêm de:

- variáveis de folga do modelo;
- interpretação do `SolveSummary`.

---

## 13. Como a solução vira arquivo de saída

Função-chave:

- `write_results_csvs(...)`

Ela chama uma família de writers:

- `write_hydro_csv`
- `write_thermal_csv`
- `write_pumping_csv`
- `write_cmosist_csv`
- `write_process_iterations_csv`
- `write_operational_limits_csv`
- `write_operational_limit_infeasibility_csv`
- `write_network_csv`
- `write_network_infeasibility_csv`
- `write_renewable_csv`

### Conexão fundamental

Essas funções não leem arquivos de entrada.

Elas leem:

- `System`
- `SolveSummary`
- resultados agregados da simulação

Então o padrão é:

```text
System + SolveSummary + IterativeSimulationResult
    ↓
writer CSV
    ↓
arquivo em RESULTADOS/
```

### Exemplo: saída hidráulica

`write_hydro_csv` conecta:

- cadastro da usina (`System`)
- volumes e turbinamentos (`SolveSummary`)
- duais do balanço hídrico (`cmo_summary`)
- agregação de subperíodos (`StudyHorizon`)

Isso mostra bem a natureza transversal da camada de simulação:

ela junta pedaços de várias partes do projeto para produzir um relatório interpretável.

---

## 14. Como um conceito atravessa todas as camadas: exemplo com renováveis

Este é um bom exemplo concreto de conexão ponta a ponta.

### Etapa 1. Entrada

Arquivos:

- `CAD_REN.csv`
- `OPER_REN.csv`

Lidos em:

- `read_renewable_catalog_table`
- `read_renewable_operation_table`

Montados em:

- `build_renewables`

### Etapa 2. Domínio

Virando:

- `RenewablePlant`

e entrando em:

- `System.renewable_plants`

### Etapa 3. Indexação

Virando:

- `Indexing.renewable_plant_entries`

### Etapa 4. Variável

Virando:

- `Variables.renewable_generation`

### Etapa 5. Restrição

Entrando no:

- balanço de demanda

### Etapa 6. Solver

Virando variável do solver com nome:

```text
renewable_generation[p=...,t=...]
```

### Etapa 7. Saída

Lida em:

- `write_renewable_csv`

e impressa em:

- `resultado_renovaveis.csv`

### Moral do exemplo

Um único conceito relevante normalmente toca:

- `io`
- `core`
- `model/indexing`
- `model/variables`
- `model/constraints`
- `solver`
- `simulation`

Por isso, quando alguma coisa “parece não funcionar”, a investigação quase sempre precisa seguir o conceito por essas camadas.

---

## 15. Como um conceito atravessa todas as camadas: exemplo com FPHA

### Etapa 1. Entrada

Arquivo:

- `OPER_FPHA.csv`

Lido em:

- `read_fpha_table`

Montado em:

- `HydroFphaSegment`

### Etapa 2. Domínio

Os segmentos entram em:

- `HydroPlant.fpha_segments`

### Etapa 3. Restrição

Em `constraints.rs`, eles alimentam:

- `build_hydro_fpha_constraints`
- `build_hydro_generation_turbining_coupling_constraints`

### Etapa 4. Saída derivada

Na simulação, a FPHA também aparece no cálculo de:

- `GeracaoMaxPontoMW`

Isso é importante porque mostra que um conceito pode aparecer:

- como restrição do modelo;
- como cálculo de relatório;
- como dado estrutural do domínio.

---

## 16. Como um conceito atravessa todas as camadas: exemplo com limites operativos

### Entrada

Arquivo:

- `OPER_REST_LIM.csv`

### IO

Lido e transformado em:

- `OperationalLimit`

### Core

O `System` passa a carregar:

- `operational_limits`

### Model

`build_operational_limit_constraints` monta as restrições correspondentes.

### Solver

As folgas de inviabilidade dessas restrições viram variáveis adicionais.

### Simulation

Essas folgas são lidas para produzir:

- `resultado_rest_lim.csv`
- `resultado_inviabilidade_lim.csv`

Então esse conceito atravessa:

```text
arquivo de entrada
    ↓
System.operational_limits
    ↓
restrições lineares
    ↓
folgas
    ↓
relatórios de inviabilidade
```

---

## 17. Como ler o projeto quando você estiver depurando algo

Se um conceito estiver errado no resultado final, siga esta ordem:

### Pergunta 1. O dado entrou corretamente?

Verifique `study.rs`.

### Pergunta 2. O dado está no `System`?

Verifique `system.rs` e o objeto construído.

### Pergunta 3. O índice dessa entidade está correto?

Verifique `indexing.rs`.

### Pergunta 4. A variável foi criada?

Verifique `variables.rs`.

### Pergunta 5. A restrição ou objetivo usa essa variável?

Verifique `constraints.rs` e `objective.rs`.

### Pergunta 6. O solver está recebendo essa variável?

Verifique `collect_variables` em `labdessem-solver`.

### Pergunta 7. O relatório está lendo a chave certa?

Verifique `labdessem-simulation/src/lib.rs`.

Essa sequência costuma reduzir muito a confusão.

---

## 18. Resumo das conexões mais importantes

### Conexão 1

```text
main.rs
    ↓
read_study_from_config
    ↓
System
```

### Conexão 2

```text
System
    ↓
Model::from_system
    ↓
Variables + ConstraintSet + Objective
```

### Conexão 3

```text
Model
    ↓
solve_model
    ↓
SolveSummary
```

### Conexão 4

```text
SolveSummary + System
    ↓
write_results_csvs
    ↓
RESULTADOS/*.csv
```

### Conexão 5

```text
nome da restrição / nome da variável
    ↓
solver
    ↓
hash maps da solução
    ↓
relatórios e duais
```

---

## 19. Fechamento

Se você quiser entender o projeto profundamente, pense assim:

- `io` transforma arquivo em dados confiáveis;
- `core` define o que esses dados significam;
- `model` transforma significado em álgebra;
- `solver` transforma álgebra em solução;
- `simulation` transforma solução em processo e relatórios.

Então a pergunta “como cada coisa se conecta?” pode ser respondida com uma regra simples:

> quase tudo no projeto nasce em `System`, vira índice, vira variável ou parâmetro de restrição, passa pelo solver e volta como saída interpretada pela simulação.

Se quiser continuar este material depois, os melhores próximos anexos seriam:

- diagrama por tipo de usina;
- mapa específico das variáveis;
- mapa específico das restrições;
- mapa dos arquivos de saída e quais variáveis os alimentam.
