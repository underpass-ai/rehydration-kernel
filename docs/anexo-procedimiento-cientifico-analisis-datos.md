# Anexo — Cómo hacer un análisis científico de datos experimentales
## Procedimiento correcto aplicado a benchmarks de agentes, contexto y rehidratación

## 1. Objetivo del anexo

Este documento explica **cómo debe hacerse un análisis científico de datos** en un benchmark experimental como el que estás construyendo.

No se centra en una implementación concreta, sino en el **procedimiento correcto** para:

- formular hipótesis,
- diseñar el experimento,
- recoger datos,
- analizarlos,
- interpretar resultados,
- y comunicar conclusiones sin sobreafirmar.

El objetivo es evitar un error muy común en proyectos técnicos:

> tener muchas tablas, gráficos y métricas, pero no tener todavía evidencia científica sólida.

---

## 2. Qué significa “análisis científico” en este contexto

Un análisis científico no consiste solo en calcular medias o dibujar gráficas.

Consiste en seguir un proceso donde:

1. se plantea una **pregunta clara**;
2. se define una **hipótesis falsable**;
3. se diseña un experimento que pueda refutarla o sostenerla;
4. se controla el máximo posible de fuentes de confusión;
5. se mide con criterios explícitos;
6. se analiza la incertidumbre;
7. se distinguen claramente:
   - observación,
   - interpretación,
   - y conclusión.

Dicho de otra forma:

> el análisis científico no busca “probar que tengo razón”; busca construir evidencia suficientemente buena para decidir qué es razonable creer.

---

## 3. Estructura correcta del procedimiento científico

## 3.1. Definir la pregunta de investigación

La primera fase consiste en formular la pregunta exacta.

### Mal planteado
- “¿Mi sistema es mejor?”
- “¿La rehidratación funciona?”
- “¿El benchmark sale bien?”

### Bien planteado
- “¿Los contextos explanatory mejoran `restart accuracy` frente a structural bajo el mismo prompt y la misma pareja agente/juez?”
- “¿El bundle multi-resolución preserva mejor el hilo causal bajo budgets pequeños?”
- “¿El planner `ResumeFocused` mejora la recuperación del punto de reanudación frente a `ReasonPreserving` en tareas de continuidad operativa?”

Una buena pregunta tiene que ser:

- concreta,
- operativa,
- medible,
- y ligada a variables observables.

---

## 3.2. Formular hipótesis

Después de la pregunta, se define una hipótesis.

### Ejemplo
- **H1**: Los contextos `explanatory` producen mayor `restart accuracy` que los contextos `structural`.
- **H2**: El modo `ResumeFocused` mejora `restart accuracy` pero reduce `reason preservation`.
- **H3**: El prompt `citation-agent` reduce errores de grounding frente a `default`.

Cada hipótesis debe ser:

- falsable,
- específica,
- y ligada a una comparación concreta.

### Importante
Siempre debe existir también una **hipótesis nula**.

Ejemplo:
- **H0**: No hay diferencia relevante entre `explanatory` y `structural` en `restart accuracy`.

Esto es importante porque evita construir un análisis orientado solo a confirmar expectativas.

---

## 3.3. Definir variables

Toda investigación experimental debe distinguir claramente tres tipos de variables.

### A. Variables independientes
Son las que manipulas.

Ejemplos en tu caso:
- tipo de contexto (`structural`, `mixed`, `explanatory`)
- modo de rehidratación
- tipo de prompt
- modelo agente
- modelo juez
- ruido (`clean`, `competing`, etc.)
- escala (`micro`, `meso`, `stress`)

### B. Variables dependientes
Son las que mides.

Ejemplos:
- `task_success`
- `restart_accuracy`
- `reason_preservation`
- latencia
- número de tokens
- estabilidad del bundle

### C. Variables de control
Son las que mantienes constantes para no contaminar el experimento.

Ejemplos:
- mismo dataset base
- mismo template de judge
- mismo budget
- misma semilla si haces comparación aislada
- mismo número de nodos
- misma estructura de ruido

Si no controlas bien estas variables, el análisis deja de ser interpretable.

---

## 3.4. Diseñar el experimento

Aquí se decide cómo se va a recoger la evidencia.

## Diseño factorial
Cuando hay varias variables independientes, lo correcto suele ser usar un diseño factorial.

Ejemplo:

- 3 mixes
- 2 noise modes
- 3 escalas
- 5 prompts
- 4 pares agente/juez

Eso crea una matriz experimental.

### Regla clave
Cada celda del diseño experimental debe ser:

- claramente identificable,
- repetible,
- comparable con las demás.

### Error común
Tener una matriz grande pero solo **una ejecución por celda**.

Eso sirve para exploración, pero no para hablar de robustez.

---

## 3.5. Definir replicación

La replicación es uno de los puntos más importantes del método científico.

### Qué significa replicar
Ejecutar la **misma condición experimental** varias veces para estimar variabilidad.

### Por qué es imprescindible
Porque en sistemas con modelos generativos hay variabilidad por:

- muestreo,
- prompts,
- orden,
- latencia,
- diferencias del judge,
- ruido del sistema.

### Regla práctica
Como mínimo:

- **3 repeticiones por celda**

Mejor aún:

- **5 o más** si quieres hablar de estabilidad.

Sin replicación, solo puedes hablar de:

- tendencia observada,
- no de robustez.

---

## 3.6. Definir métricas correctamente

Las métricas deben representar bien el fenómeno que quieres medir.

### Requisitos de una buena métrica
- claramente definible,
- consistente,
- reproducible,
- y alineada con la pregunta.

### Ejemplo de mala métrica
Una métrica binaria `reason_preserved=yes/no` que acepta razones plausibles aunque no sean correctas.

### Ejemplo de mejor diseño
Separar:

- `reason_correct_main_path`
- `reason_plausible_but_wrong`
- `reason_missing`
- `reason_contradictory`

Esto convierte una métrica ambigua en una observación científicamente más útil.

### Regla importante
Si una métrica no distingue correctamente entre:
- correcto,
- incorrecto,
- plausible,
- parcialmente correcto,

entonces el análisis será débil aunque los números parezcan limpios.

---

## 3.7. Asegurar validez del benchmark

Aquí entran los conceptos de validez.

## A. Validez interna
Pregunta:

> ¿La diferencia observada viene realmente de la variable que quería estudiar?

Ejemplo:
si cambias a la vez contexto, prompt y judge, no sabes qué causó qué.

## B. Validez externa
Pregunta:

> ¿Esto generaliza fuera de este benchmark?

Ejemplo:
un resultado fuerte en grafos sintéticos no implica el mismo resultado en tareas reales de agentes.

## C. Validez de constructo
Pregunta:

> ¿La métrica que uso representa realmente el fenómeno que digo medir?

Ejemplo:
si `reason_preserved` acepta respuestas distractoras, entonces no mide bien “preservación de razón correcta”.

## D. Validez de conclusión
Pregunta:

> ¿Las conclusiones estadísticas están justificadas por el diseño y el análisis?

Ejemplo:
si solo hay una ejecución por celda, no puedes afirmar robustez estadística fuerte.

---

## 3.8. Inspección de calidad de datos antes de analizar

Antes de calcular medias o hipótesis hay que validar los datos.

### Comprobaciones mínimas
- número total de filas esperado
- distribución por condición
- ausencia de celdas vacías
- ausencia de duplicados accidentales
- detección de valores imposibles
- consistencia de etiquetas
- trazabilidad entre run y condición experimental

### También hay que revisar:
- logs
- ejemplos concretos
- outputs individuales
- no solo agregados

Esto es crítico.

Muchos errores metodológicos se detectan viendo ejemplos concretos del benchmark y no solo tablas resumen.

---

## 3.9. Hacer análisis descriptivo primero

Antes de correr pruebas o sacar conclusiones, hay que hacer análisis descriptivo.

### Qué incluye
- medias
- medianas
- distribuciones
- tablas por factor
- tablas cruzadas
- visualización de patrones fuertes

### Objetivo
Entender el comportamiento global de los datos antes de interpretar causalidad.

### Importante
El análisis descriptivo no demuestra causalidad.  
Solo muestra patrones.

---

## 3.10. Analizar incertidumbre

Aquí está una de las diferencias entre “dashboard” y “análisis científico”.

No basta con reportar medias.  
Hay que reportar incertidumbre.

### Herramientas habituales
- desviación estándar
- error estándar
- intervalos de confianza
- distribución por réplica
- bootstrap
- análisis bayesiano si aplica

### Advertencia importante
Un intervalo de confianza sobre una sola muestra agregada **no sustituye** la falta de replicación experimental.

Ejemplo:
si tienes una sola observación por celda, el intervalo sobre la tasa global puede ser matemáticamente correcto, pero no te dice nada sólido sobre estabilidad entre ejecuciones.

---

## 3.11. Analizar factores de confusión

Siempre hay que preguntarse:

> ¿Podría haber otra explicación para este resultado?

Ejemplos de confusión en tu tipo de benchmark:
- prompt más importante que contexto
- judge permisivo
- ruido que introduce pistas útiles
- pares agente/juez no comparables
- dataset mal balanceado
- distractores semánticamente contaminados

El análisis científico exige intentar falsar la explicación más cómoda.

---

## 3.12. Hacer contraste de hipótesis con prudencia

Solo después de todo lo anterior tiene sentido hacer contraste formal.

### Qué comparar
- explanatory vs structural
- mixed vs structural
- planner A vs planner B
- prompt X vs prompt Y

### Qué necesitas para hacerlo bien
- suficiente replicación
- métricas bien definidas
- diseño limpio
- supuestos razonables

### Si no tienes eso
No pasa nada, pero entonces debes hablar de:

- evidencia exploratoria,
- no de prueba fuerte.

---

## 3.13. Distinguir claramente resultados, interpretación y conclusión

Esta separación es crítica.

### Resultado
Dato observado.

Ejemplo:
- “El promedio de `restart` en `explanatory` es 0.458 y en `structural` es 0.188”.

### Interpretación
Lectura razonable del dato.

Ejemplo:
- “Esto sugiere que el contexto explanatory facilita la localización del punto de reanudación”.

### Conclusión
Afirmación final con alcance controlado.

Ejemplo:
- “En este benchmark sintético, explanatory supera a structural en restart, pero la ausencia de replicación limita la robustez de la afirmación”.

El error típico es saltar del resultado a una conclusión demasiado ambiciosa.

---

## 3.14. Comunicar limitaciones explícitamente

Todo informe serio debe decir claramente qué no demuestra.

### Debe incluir
- limitaciones del dataset
- limitaciones del diseño
- limitaciones de la métrica
- limitaciones de generalización
- posibles sesgos del judge
- límites del tamaño muestral
- deuda metodológica pendiente

Esto no debilita el trabajo.  
Lo fortalece, porque lo hace creíble.

---

## 4. Aplicación práctica a benchmarks de agentes y rehidratación

En un benchmark como el tuyo, el procedimiento correcto sería:

### Paso 1
Definir preguntas concretas:

- ¿El contexto explanatory mejora la continuidad operativa?
- ¿El bundle multi-resolución mejora la preservación causal bajo token pressure?
- ¿El modo `ResumeFocused` mejora la reanudación?

### Paso 2
Definir variables independientes:

- context mix
- planner mode
- prompt
- modelo agente
- modelo juez
- budget
- noise mode
- graph scale

### Paso 3
Definir métricas mejores:

- `task_success`
- `restart_exact`
- `restart_off_by_one`
- `reason_correct_main_path`
- `reason_plausible_but_wrong`
- `token_efficiency`
- `bundle_stability`

### Paso 4
Crear benchmark sin contaminación semántica

Por ejemplo:

- `structural` de verdad sin rationale textual
- `competing_restart`
- `conflicting_reason`
- `irrelevant_noise`

### Paso 5
Replicar cada condición

Como mínimo:
- 3 seeds por celda

### Paso 6
Hacer análisis descriptivo

### Paso 7
Hacer análisis inferencial prudente

### Paso 8
Reportar:
- resultados
- incertidumbre
- limitaciones
- interpretación
- y próximas correcciones del benchmark

---

## 5. Qué errores hay que evitar siempre

## Error 1
Confundir muchas filas con buena ciencia.

Tener 720 filas no implica buena evidencia si la estructura experimental tiene una sola observación por condición.

## Error 2
Confundir gráficos bonitos con robustez.

Muchas tablas y gráficas ayudan a leer, pero no sustituyen:
- replicación,
- control de variables,
- ni buena definición de métricas.

## Error 3
Aceptar una métrica mal definida.

Una métrica ambigua contamina todo el análisis posterior.

## Error 4
No inspeccionar ejemplos concretos.

Mirar solo agregados impide detectar:
- leakage,
- contaminación semántica,
- judge permisivo,
- errores de etiquetado.

## Error 5
Sobreafirmar.

La frase correcta muchas veces es:
- “sugiere”
- “apunta a”
- “muestra evidencia direccional”
- “todavía no demuestra robustez”

Eso es ciencia honesta.

---

## 6. Plantilla práctica para informes futuros

## A. Research question
Una frase clara.

## B. Hypothesis
Hipótesis principal e hipótesis nula.

## C. Experimental design
Factores, niveles, celdas, número de repeticiones.

## D. Metrics
Definición exacta de cada métrica.

## E. Data quality checks
Validación previa de integridad.

## F. Descriptive results
Tablas y figuras.

## G. Uncertainty analysis
Replicación, varianza, intervalos.

## H. Threats to validity
Interna, externa, constructo, conclusión.

## I. Interpretation
Lectura prudente.

## J. Conclusion
Qué se puede afirmar y qué no.

---

## 7. Criterio de calidad para decir “esto ya es evidencia fuerte”

Yo no diría que un benchmark ofrece evidencia fuerte hasta que cumpla, como mínimo, esto:

1. condiciones limpias y bien separadas;
2. métricas no ambiguas;
3. replicación por celda;
4. diseño sin contaminación evidente;
5. revisión de outputs individuales;
6. análisis de incertidumbre;
7. limitaciones explícitas;
8. conclusiones proporcionadas al diseño.

Si falta alguna de las tres primeras, el benchmark sigue estando en fase exploratoria.

---

## 8. Conclusión final

El procedimiento científico correcto no consiste en acumular números.  
Consiste en construir evidencia fiable, trazable y falsable.

Aplicado a benchmarks de rehidratación y agentes, eso exige:

- diseñar bien el benchmark,
- definir bien las métricas,
- replicar,
- revisar ejemplos concretos,
- analizar incertidumbre,
- y comunicar con humildad metodológica.

La regla práctica más útil es esta:

> **primero limpiar el benchmark, luego medir, luego interpretar, y solo al final concluir.**

Ese orden no es burocracia.  
Es lo que separa un experimento útil de una narrativa basada en números.
