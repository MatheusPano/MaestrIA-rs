Cockpit para sessões do Claude Code, reescrito em Rust. Instala como
`maestria-rs`, ao lado da versão em Flutter, sem substituí-la.

## Baixar

| | arquivo |
|---|---|
| **macOS** (Intel e Apple Silicon) | `maestria-rs_*_macos.dmg` |
| **Linux** (Ubuntu 22.04+, Mint, Pop!_OS) | `maestria-rs_*_amd64.deb` |
| **Windows** 10 1809+ / 11 (x64) | `maestria-rs_*_x64.msi` |

## Antes de tudo: o Claude Code

O maestria não substitui o `claude` — ele abre sessões do CLI em painéis e
escuta os hooks delas. Sem o Claude Code instalado e logado com a **sua** conta,
o app abre e não sobe sessão nenhuma.

```
curl -fsSL https://claude.ai/install.sh | bash
claude          # loga na primeira execução
```

## macOS

Abra o `.dmg`, arraste o **maestria-rs** pro Applications. Na primeira abertura o
macOS vai reclamar: o app é assinado *ad-hoc*, sem conta paga de desenvolvedor
da Apple, então ele não passa pelo Gatekeeper de um download. Uma vez, no
terminal:

```
xattr -dr com.apple.quarantine /Applications/maestria-rs.app
```

Depois disso abre normal, e não se reclama de novo. (Alternativa pelo mouse:
**Ajustes do Sistema → Privacidade e Segurança**, e "Abrir Assim Mesmo" no
aviso que aparece logo depois da tentativa recusada.)

## Linux

```
sudo apt install ./maestria-rs_*_amd64.deb
```

O apt puxa o que falta. Depois é "MaestrIA (Rust)" no menu de aplicativos, ou
`maestria-rs` no terminal.

## Windows

Precisa do [Git for Windows](https://git-scm.com/download/win) (o Git Bash é
por onde o app roda git e o `claude`, como o próprio Claude Code exige) e do
Claude Code (`irm https://claude.ai/install.ps1 | iex`). Rode o `.msi`; o
SmartScreen avisa na primeira vez porque o pacote não é assinado: **Mais
informações → Executar assim mesmo**. Painel de shell abre o `pwsh`, ou o
Windows PowerShell se ele não estiver instalado.

## Pelo cargo (direto do git)

Compila na sua máquina (uns 15 minutos) e serve pra qualquer distro ou pro Mac.
Precisa do [Rust](https://rustup.rs) e, no Linux, das bibliotecas de sistema
(no Ubuntu/Debian):

```
sudo apt install build-essential pkg-config clang cmake libxkbcommon-dev \
  libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-dev libx11-xcb-dev \
  libxcb1-dev libxi-dev libxcursor-dev libxrandr-dev libfontconfig-dev \
  libfreetype-dev libssl-dev libzstd-dev libasound2-dev
cargo install --locked --git https://github.com/MatheusPano/MaestrIA-rs --tag {TAG} mx-app
```

O binário fica em `~/.cargo/bin/maestria`. Esse caminho não cria atalho no menu
nem ícone, e o mesmo nome `maestria` pode encobrir o da versão em Flutter no
PATH. No Linux com ibus, rode `XMODIFIERS= maestria` (o `.deb` já faz isso)
pra evitar letras duplicadas sob carga.

## Atualizações

O app pode se atualizar sozinho, se você ligar em **Configurações →
Atualizações** (vem desligado). Ligado, quando sai versão nova ele baixa em
segundo plano e mostra o número dela na barra de baixo, ao lado do sino. No Mac
ela entra quando você fecha o app; no Windows também (o instalador pede
permissão de administrador); no Linux, quando você clica (o sistema pede a
senha pra instalar o pacote). Nada reinicia sem você pedir. Vale só pro app
instalado pelo `.deb`, `.dmg` ou `.msi` da release.

## Primeiro uso

Não vem com pasta nenhuma: **adicionar pasta** e aponte pra um repo git seu. As
configurações ficam em `~/.maestria-rs/config.json`.
