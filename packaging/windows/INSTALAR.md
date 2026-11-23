# maestria no Windows

## 1. Git for Windows e Claude Code primeiro

O maestria abre sessões do `claude` em painéis e roda os comandos dele (git,
worktrees, diffs) pelo Git Bash — o mesmo que o Claude Code exige no Windows.
Instale os dois e logue com a **sua** conta:

```powershell
winget install --id Git.Git -e
irm https://claude.ai/install.ps1 | iex
claude          # loga na primeira execução
```

Se o Git Bash não estiver em `C:\Program Files\Git\bin\bash.exe` nem ao lado do
`git.exe` do PATH, aponte `CLAUDE_CODE_GIT_BASH_PATH` para ele.

## 2. Instalar

Rode o `maestria-rs_<versão>_x64.msi`. Ele instala em
`C:\Program Files\maestria-rs` e cria o atalho no menu Iniciar. As
atualizações seguintes o próprio app baixa e instala ao fechar.

## 3. O aviso do SmartScreen

O `.msi` não é assinado com certificado de código, então o SmartScreen avisa
na primeira execução ("O Windows protegeu o computador"). Clique em
**Mais informações → Executar assim mesmo**.

## O shell dos painéis

Painel de shell abre o PowerShell 7 (`pwsh`) se estiver instalado, senão o
Windows PowerShell.
