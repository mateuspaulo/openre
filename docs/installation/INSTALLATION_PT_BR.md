# Guia de Instalação e Uso do OpenRe (Português do Brasil)

Seja bem-vindo ao **OpenRe**, um aplicativo de código aberto, gratuito e voltado à privacidade para recuperação de arquivos deletados e análise forense de discos no Windows e Ubuntu Linux.

---

## 1. Requisitos do Sistema

### Windows
- **Sistema Operacional:** Windows 10 (64 bits) ou Windows 11 (64 bits).
- **Componente Visual:** Microsoft Edge WebView2 (já vem pré-instalado por padrão no Windows 11 e na maioria das versões do Windows 10).
- **Dependências Externas:** Nenhuma! A versão portátil (`OpenRe-Windows-x64-Portable.zip`) é totalmente independente e não precisa de instaladores adicionais.

### Ubuntu Linux
- **Sistema Operacional:** Ubuntu 22.04 LTS, Ubuntu 24.04 LTS ou qualquer distribuição derivada do Debian (64 bits).
- **Pacotes do Sistema:** `libwebkit2gtk-4.1` (ou `libwebkit2gtk-4.0`) e bibliotecas gráficas padrão do GNOME/X11.

---

## 2. Instruções de Instalação e Inicialização

### 2.1 No Windows (Versão Portátil)
1. Baixe o arquivo `OpenRe-Windows-x64-Portable.zip` na página oficial de Releases do GitHub.
2. Descompacte o arquivo ZIP em uma pasta segura (por exemplo, em um pendrive ou em uma partição diferente daquela com os arquivos perdidos).
   > **REGRA DE OURO:** NUNCA descompacte ou execute o OpenRe no mesmo disco ou partição de onde você deseja recuperar arquivos! A simples extração de arquivos pode sobrescrever dados excluídos.
3. Dê um duplo clique no arquivo `OpenRe.exe` para abrir a interface gráfica.

### 2.2 No Ubuntu Linux (Pacote .deb)
1. Baixe o pacote `openre_amd64.deb`.
2. Abra o terminal na pasta do arquivo e instale com o comando:
   ```bash
   sudo apt update
   sudo apt install ./openre_amd64.deb
   ```
3. Abra o aplicativo pelo menu do sistema ou digite no terminal:
   ```bash
   openre
   ```

---

## 3. Permissões de Acesso e Modo Administrador

- **Leitura de Pastas e Imagens de Disco (`.img`, `.dd`):** Não exige privilégios de administrador. Você pode escanear arquivos de backup e imagens de disco diretamente.
- **Leitura Direta de Discos Físicos e Partições do Sistema:** O Windows e o Linux protegem o acesso a blocos brutos de discos para evitar falhas de segurança. Para escanear um disco rígido físico diretamente (`\\.\PhysicalDrive0` ou `/dev/sdb`), execute o OpenRe como Administrador (no Windows: botão direito em `OpenRe.exe` > *Executar como Administrador*; no Linux: execute com `sudo`).

---

## 4. Passo a Passo para uma Recuperação Segura

![Dashboard Inicial do OpenRe](../screenshots/01_dashboard.png)

O OpenRe guia você em 7 etapas práticas:

1. **Selecionar a Origem:** Escolha a partição, disco físico, pasta ou imagem de disco (`.img` / `.dd`) onde estavam os arquivos apagados.
2. **Método de Varredura:**
   - **Varredura Rápida (Quick Scan):** Analisa a estrutura do sistema de arquivos (NTFS, FAT32, exFAT, ext4) em busca de registros marcados como excluídos. Mantém os nomes originais, pastas e datas.
   - **Varredura Profunda (Deep Scan):** Examina bloco a bloco a mídia procurando assinaturas digitais de arquivos (JPEG, PNG, PDF, DOCX, ZIP, MP4, etc.). Ideal para partições formatadas ou discos corrompidos.
3. **Filtros:** Filtre previamente por categoria (Imagens, Documentos, Vídeos, Áudio) ou tamanho se desejar agilizar a busca.
4. **Varredura:** Acompanhe o progresso em tempo real, velocidade de leitura, arquivos encontrados e integridade detectada.
5. **Pré-Visualização (Preview):** Visualize miniaturas de fotos, visualizador hexadecimal de blocos e metadados antes de iniciar a extração.
   
   ![Revisão e Pré-visualização de Arquivos](../screenshots/03_review_preview.png)

6. **Destino da Recuperação:** **ATENÇÃO:** Escolha SEMPRE um disco diferente do disco de origem (por exemplo, recupere de `C:` para `D:` ou para um pendrive externo). O OpenRe bloqueia por padrão a seleção do mesmo disco para proteger seus dados contra sobrescrita permanente.
7. **Relatório Final:** Visualize o resumo completo com quantidade de bytes restaurados, hashes de integridade e possíveis avisos de arquivos truncados.

---

## 5. Boas Práticas e Resolução de Problemas

- **O que fazer se um arquivo estiver corrompido?**  
  O módulo de reparo do OpenRe pode corrigir cabeçalhos de imagens danificadas (como falta do marcador EOI em JPEG ou cálculo de CRC em PNG), gerando sempre uma cópia reparada sem alterar o arquivo original recuperado.
- **Por que alguns arquivos não podem ser recuperados?**  
  Se novos arquivos foram gravados no mesmo local, ou se o disco utiliza tecnologia SSD com recurso TRIM ativado, os blocos podem ter sido permanentemente zerados pelo controlador do hardware.
- **O dispositivo emite estalos ou barulhos mecânicos?**  
  Interrompa imediatamente qualquer varredura de software. Problemas físicos exigem serviços laboratoriais especializados de recuperação em sala limpa.
