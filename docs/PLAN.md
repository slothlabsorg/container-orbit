# orbit — Plan de implementación

## Contexto

Trabajar con Docker en la laptop (este Mac) consume RAM/CPU/disco y la mata. Hay
máquinas más potentes en la LAN (otro Mac, un PC gamer Windows, una caja Linux)
que pueden cargar ese trabajo. El objetivo: **un comando en cada máquina** y a
partir de ahí toda operación normal de Docker (`build`, `run`, `compose`, `pull`)
se ejecuta en la máquina remota, mientras en la laptop sigues trabajando como si
nada — incluyendo poder acceder a los puertos publicados (`-p 8080:80`) en tu
`localhost` local.

`orbit` es esa herramienta: un CLI en Rust, 100% terminal, compatible con
cualquier motor que respete `docker context` (Docker Desktop, Rancher Desktop,
OrbStack, colima). Diseñado para crecer (como minikube) hacia sync de código,
múltiples hosts y Windows nativo.

Foco de entrega: **Mac→Mac primero**, luego **Mac→Windows (WSL2)**, Linux sale gratis.

## Idea central (por qué funciona)

Docker ya soporta daemons remotos vía `docker context` + `DOCKER_HOST=ssh://`.
Eso resuelve el 90% (build/run/disco/RAM viven en el host). Lo que falta para que
sea usable a diario y es el valor real de `orbit`:

1. **Setup automático** — un comando por lado, idempotente.
2. **Reenvío de puertos automático** — cuando corres `-p 8080:80`, el puerto queda
   en el host remoto, no en la laptop. `orbit` observa los eventos del daemon
   remoto y abre/cierra túneles SSH `-L` para que `curl localhost:8080` en la
   laptop pegue al contenedor en el gamer. Esto es lo que lo hace transparente.
3. **Diagnóstico** — `orbit doctor` dice qué está mal y cómo arreglarlo.

## Roles

- **Cliente** (laptop): redirige el `docker` local al host y reenvía puertos de vuelta.
- **Host** (gamer / otro Mac): corre el motor Docker real y expone su socket por SSH.

## Superficie del CLI (clap)

| Comando | Dónde | Qué hace |
|---|---|---|
| `orbit host init` | host | Verifica Docker corriendo + SSH server activo; autoriza la llave `orbit`; imprime el *join string* (`user@ip` + huella + ruta del socket). Idempotente. |
| `orbit link <join\|user@host>` | cliente | Genera/instala llave SSH, prueba SSH + docker remoto, crea `docker context orbit` (endpoint `ssh://user@host`), guarda `~/.config/orbit/config.toml`. |
| `orbit up [--foreground]` | cliente | `docker context use orbit`, abre conexión SSH maestra multiplexada, reenvía el socket docker remoto a un socket unix local, arranca el daemon reconciliador de puertos. |
| `orbit down` | cliente | Restaura el contexto docker previo, cierra forwards y la conexión maestra. |
| `orbit status` | cliente | Host vinculado, estado, contexto actual, puertos reenviados (contenedor→local), uso de recursos remoto. |
| `orbit ports [add\|rm <port>]` | cliente | Lista forwards activos; alta/baja manual de forwards TCP (servicios no-docker). |
| `orbit doctor` | ambos | Diagnóstico accionable: SSH, docker remoto, socket forward, contexto, reloj. |

## Mecanismo

- **Transporte:** SSH (OpenSSH; presente en macOS/Linux/Windows). Una conexión
  **maestra multiplexada** (`ControlMaster`/`ControlPath`) que comparten todos los forwards.
- **Redirección de docker:** vía `docker context use orbit` (endpoint `ssh://`).
  No envolvemos `docker`; usamos contextos estándar → compatibilidad nativa con
  Docker Desktop / Rancher / OrbStack / colima.
- **Auto port-forward (núcleo):** `orbit` reenvía el socket docker remoto a
  `~/.config/orbit/run/docker.sock` (forward unix→unix por SSH). Se conecta con el
  crate `bollard`, se suscribe a `/events`, lista contenedores corriendo, calcula
  los puertos publicados y por cada uno abre `ssh -O forward -L <port>:127.0.0.1:<port>`
  sobre el control socket. Al parar el contenedor: `ssh -O cancel`. Reconcilia en
  cada evento.

## Adaptadores de host (extensibilidad)

`trait HostAdapter` abstrae cómo localizar/exponer el socket docker remoto:

- `UnixSocketHost` (macOS, Linux) — socket en `/var/run/docker.sock` o ruta
  detectada de OrbStack/Rancher. **v1, completo — cubre Mac→Mac.**
- `WindowsWslHost` — socket dentro de WSL2; SSH ejecuta `wsl ...` como puente. **v1.1 — Mac→Windows.**
- `WindowsNativeHost` — named pipe vía relay pequeño. **futuro.**

`orbit host init` detecta y registra el adaptador en config.

## Layout

```
container-orbit/
├─ Cargo.toml
├─ README.md
├─ docs/{PLAN.md, ARCHITECTURE.md, ROADMAP.md}
└─ src/
   ├─ main.rs            dispatch clap
   ├─ cli.rs             definición de comandos/args
   ├─ config.rs          ~/.config/orbit/config.toml
   ├─ ssh.rs             conexión maestra, llaves, -O forward/cancel
   ├─ docker_ctx.rs      crear/usar/restaurar docker context (shell a `docker`)
   ├─ forwarder.rs       loop de eventos bollard + reconciliador
   ├─ host/{mod.rs, unix.rs, windows_wsl.rs}
   ├─ commands/{host_init,link,up,down,status,ports,doctor}.rs
   └─ util.rs            spawn de procesos, errores, salida con color
```

## Dependencias

`clap` (derive), `bollard`, `tokio`, `serde`+`toml`, `anyhow`/`thiserror`,
`tracing`(+subscriber), `owo-colors`. v1 hace *shell-out* a `ssh` y `docker`
(robusto, menos código) tras wrappers finos, para poder migrar luego a `russh`.

## Fases / tareas

1. Scaffold cargo + esqueleto CLI + config.
2. Módulo `ssh` (keygen, maestra, forward/cancel) + `docker_ctx`.
3. `host init` (adaptador unix) + `link`.
4. `forwarder` (eventos bollard → reconciliar forwards) + `up`/`down`.
5. `status` / `ports` / `doctor`.
6. Docs (README, ARCHITECTURE, ROADMAP) + stub adaptador Windows/WSL.

## Verificación (Mac→Mac, una sola máquina como loopback)

Con "Remote Login" activado, este Mac actúa como su propio host vía `ssh localhost`:

1. `orbit host init` → imprime join string.
2. `orbit link dany@localhost` → crea contexto `orbit`.
3. `orbit up` → maestra + socket forward + daemon.
4. `docker run -d -p 8080:80 nginx` (corre vía contexto) → `curl localhost:8080` OK a través del forward.
5. `orbit status` muestra el puerto reenviado; `orbit down` restaura el contexto.
6. `cargo build` + `cargo clippy` limpios.

Test de dos máquinas reales documentado en README.
