Cockpit para sessões do Claude Code, reescrito em Rust. Instala como
`maestria-rs`, ao lado da versão em Flutter, sem substituí-la.

## Baixar

| | arquivo |
|---|---|
| **macOS** (Intel e Apple Silicon) | `maestria-rs_*_macos.dmg` |
| **Linux** (Ubuntu 22.04+, Mint, Pop!_OS) | `maestria-rs_*_amd64.deb` |

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

## Primeiro uso

Não vem com pasta nenhuma: **adicionar pasta** e aponte pra um repo git seu. As
configurações ficam em `~/.maestria-rs/config.json`.
