//! Portuguese (Brazil) texts: (English, Portuguese). The English text is
//! the one in the code; `cargo test` fails if one is missing here.

pub static TEXTS: &[(&str, &str)] = &[
    ("1 kHz square wave, CH1", "Onda quadrada de 1 kHz, CH1"),
    (
        "DPOX180H, 500 µs/div, CH1 1 V/div DC: a 1 kHz square wave of about 1.6 Vpp. 48000 samples at 4 MS/s; trigger Auto, rising edge on CH1.",
        "DPOX180H, 500 µs/div, CH1 1 V/div DC: uma onda quadrada de 1 kHz com cerca de 1,6 Vpp. 48000 amostras a 4 MS/s; gatilho Auto, borda de subida no CH1.",
    ),
    ("Two channels, CH2 in AC", "Dois canais, CH2 em AC"),
    (
        "DPOX180H, 500 µs/div. CH1 1 V/div DC and CH2 2 V/div AC coupling, the same 1 kHz square wave (small on CH2). Try Y: Div and the channel readouts at the bottom.",
        "DPOX180H, 500 µs/div. CH1 1 V/div DC e CH2 2 V/div com acoplamento AC, a mesma onda quadrada de 1 kHz (pequena no CH2). Experimente Y: Div e as leituras dos canais embaixo.",
    ),
    (
        "1 kHz square wave, CH1 and CH2",
        "Onda quadrada de 1 kHz, CH1 e CH2",
    ),
    (
        "1013D, 500 µs/div. CH1 1 V/div DC, about 3.2 Vpp. CH2 is on with no signal. Try the measurements and the FFT (odd harmonics).",
        "1013D, 500 µs/div. CH1 1 V/div DC, cerca de 3,2 Vpp. O CH2 está ligado sem sinal. Experimente as medições e a FFT (harmônicas ímpares).",
    ),
    (
        "1 kHz square wave, clipped",
        "Onda quadrada de 1 kHz, saturada",
    ),
    (
        "1013D, 100 µs/div, CH1 0.5 V/div. The signal goes past the bottom of the screen: the voltage measurements are marked with \"?\".",
        "1013D, 100 µs/div, CH1 0,5 V/div. O sinal passa da parte de baixo da tela: as medições de tensão aparecem com \"?\".",
    ),
    ("Small AC signal, 50 mV/div", "Sinal AC pequeno, 50 mV/div"),
    (
        "1013D, 2 ms/div, AC coupling, about 36 mVpp: a low-level, noisy signal.",
        "1013D, 2 ms/div, acoplamento AC, cerca de 36 mVpp: um sinal fraco e com ruído.",
    ),
    ("1 MHz signal", "Sinal de 1 MHz"),
    (
        "1013D, 500 ns/div, CH1 1 V/div AC, about 5 Vpp.",
        "1013D, 500 ns/div, CH1 1 V/div AC, cerca de 5 Vpp.",
    ),
    ("10 MHz signal", "Sinal de 10 MHz"),
    (
        "1013D, 100 ns/div, CH1 1 V/div AC: few samples per period.",
        "1013D, 100 ns/div, CH1 1 V/div AC: poucas amostras por período.",
    ),
    ("500 kHz on two channels", "500 kHz em dois canais"),
    (
        "1013D, 1 µs/div. CH1 1 V/div and CH2 2.5 V/div, both AC. Try Y: Div and the voltage cursors.",
        "1013D, 1 µs/div. CH1 1 V/div e CH2 2,5 V/div, ambos AC. Experimente Y: Div e os cursores de tensão.",
    ),
    ("Slow signal, about 1 Hz", "Sinal lento, cerca de 1 Hz"),
    (
        "1013D, 2 s/div: a 1 minute record.",
        "1013D, 2 s/div: um registro de 1 minuto.",
    ),
    (
        "DHO1074: four channels (.bin)",
        "DHO1074: quatro canais (.bin)",
    ),
    (
        "Rigol DHO1074 .bin file, 5 ms/div, four channels with different signals. Use the channel buttons to show one at a time.",
        "Arquivo .bin do Rigol DHO1074, 5 ms/div, quatro canais com sinais diferentes. Use os botões dos canais para mostrar um de cada vez.",
    ),
    (
        "MSO5000: 1 kHz on four channels (.bin)",
        "MSO5000: 1 kHz em quatro canais (.bin)",
    ),
    (
        "Rigol MSO5000 .bin file, 500 µs/div. The 1 kHz calibration signal on four channels: CH1 and CH4 about 3.2 Vpp, CH2 and CH3 about 1 Vpp.",
        "Arquivo .bin do Rigol MSO5000, 500 µs/div. O sinal de calibração de 1 kHz em quatro canais: CH1 e CH4 com cerca de 3,2 Vpp, CH2 e CH3 com cerca de 1 Vpp.",
    ),
    (
        "DS2072A: 1 kHz, two channels (CSV)",
        "DS2072A: 1 kHz, dois canais (CSV)",
    ),
    (
        "CSV file saved by a Rigol DS2072A: the 1 kHz calibration signal, about 0.32 Vpp, on CH1 and CH2.",
        "Arquivo CSV salvo por um Rigol DS2072A: o sinal de calibração de 1 kHz, cerca de 0,32 Vpp, no CH1 e no CH2.",
    ),
    (
        "DS1102E: 4 kHz, two channels (CSV)",
        "DS1102E: 4 kHz, dois canais (CSV)",
    ),
    (
        "CSV file saved by a Rigol DS1102E: about 4.2 kHz and 6 Vpp on CH1 and CH2.",
        "Arquivo CSV salvo por um Rigol DS1102E: cerca de 4,2 kHz e 6 Vpp no CH1 e no CH2.",
    ),
    (
        "CPU bus: write and read cycles",
        "Barramento de CPU: ciclos de escrita e leitura",
    ),
    (
        "A 10 MHz bus: address, data and strobes. The data bus is z (shown X) when no device drives it. Zoom in to read the values; right-click a bus readout at the bottom for Hex, Dec, Bin or ASCII.",
        "Um barramento de 10 MHz: endereço, dados e sinais de controle. O barramento de dados fica em z (mostrado como X) quando nenhum dispositivo o comanda. Dê zoom para ler os valores; clique com o botão direito numa leitura de barramento embaixo para Hex, Dec, Bin ou ASCII.",
    ),
    (
        "8-bit DAC: code and analog output",
        "DAC de 8 bits: código e saída analógica",
    ),
    (
        "A 500 Hz sine from an 8-bit DAC: the code bus, the load strobe and the filtered analog output in volts. Try the time cursors on the code and vout.",
        "Uma senoide de 500 Hz de um DAC de 8 bits: o barramento do código, o sinal de carga e a saída analógica filtrada, em volts. Experimente os cursores de tempo no código e em vout.",
    ),
    ("UART: AT command and reply", "UART: comando AT e resposta"),
    (
        "A host sends \"AT\" on tx and the device answers \"OK\" on rx, at 115200 baud 8N1. In Decode ▾, check tx and rx; Show ASCII to read the text.",
        "Um host envia \"AT\" em tx e o dispositivo responde \"OK\" em rx, a 115200 baud 8N1. Em Decodificar ▾, marque tx e rx; Mostrar ASCII para ler o texto.",
    ),
    (
        "UART on an analog line (9600 baud)",
        "UART numa linha analógica (9600 baud)",
    ),
    (
        "A 3.3 V serial line as an oscilloscope sees it: slow edges and noise. Decode ▾ → UART finds the baud rate and the threshold; Show ASCII reads \"Hello, FNIRSI!\".",
        "Uma linha serial de 3,3 V como o osciloscópio a vê: bordas lentas e ruído. Decodificar ▾ → UART encontra o baud rate e o limiar; Mostrar ASCII lê \"Hello, FNIRSI!\".",
    ),
    (
        "UART: complete session (4 lines)",
        "UART: sessão completa (4 linhas)",
    ),
    (
        "Four serial lines with different settings. host_tx and dev_tx: AT commands and answers, 115200 8N1, then a break on dev_tx (framing error). gps: NMEA sentences, 9600 8N1. rs232: an inverted line, 19200 7E1 (Data bits 7, Parity Even, Idle Low), with one parity error. In Decode ▾, check the lines and set rs232 in its ▸ settings; Show ASCII, and zoom in on the fast lines.",
        "Quatro linhas seriais com configurações diferentes. host_tx e dev_tx: comandos AT e respostas, 115200 8N1, depois um break em dev_tx (erro de enquadramento). gps: frases NMEA, 9600 8N1. rs232: uma linha invertida, 19200 7E1 (Bits de dados 7, Paridade Par, Repouso Baixo), com um erro de paridade. Em Decodificar ▾, marque as linhas e configure rs232 nos seus ajustes ▸; Mostrar ASCII, e dê zoom nas linhas rápidas.",
    ),
    ("I2C: temperature sensor", "I2C: sensor de temperatura"),
    (
        "I2C at 100 kHz: write a register of a sensor at 0x48, read the temperature with a repeated start (0x19 0x80 = 25.5 °C, the last byte with a NACK), and an address that nobody answers (0x50, NACK). In Decode ▾, check sda: it uses I2C with scl as clock.",
        "I2C a 100 kHz: escreve um registrador de um sensor em 0x48, lê a temperatura com um start repetido (0x19 0x80 = 25,5 °C, o último byte com NACK), e um endereço que ninguém responde (0x50, NACK). Em Decodificar ▾, marque sda: ele usa I2C com scl como clock.",
    ),
    (
        "I2C on two analog channels",
        "I2C em dois canais analógicos",
    ),
    (
        "The temperature read as an oscilloscope sees it: CH1 is SDA, CH2 is SCL, 3.3 V open-drain lines with slow rising edges and noise. In Decode ▾, check CH1, then in its ▸ settings choose I2C and CH2 as SCL.",
        "A leitura de temperatura como o osciloscópio a vê: CH1 é SDA, CH2 é SCL, linhas open-drain de 3,3 V com subidas lentas e ruído. Em Decodificar ▾, marque CH1 e, nos seus ajustes ▸, escolha I2C e CH2 como SCL.",
    ),
    ("SPI: flash memory", "SPI: memória flash"),
    (
        "SPI at 1 MHz, mode 0, with cs, sck, mosi and miso: read the JEDEC ID (0x9F, answer EF 40 18), write enable (0x06), and read 9 bytes (0x03 at 0x000100): \"Hello SPI\" on miso. In Decode ▾, check mosi and miso: they use SPI with sck and cs by themselves.",
        "SPI a 1 MHz, modo 0, com cs, sck, mosi e miso: lê o JEDEC ID (0x9F, resposta EF 40 18), habilita a escrita (0x06) e lê 9 bytes (0x03 em 0x000100): \"Hello SPI\" em miso. Em Decodificar ▾, marque mosi e miso: eles usam SPI com sck e cs automaticamente.",
    ),
    (
        "SPI on two analog channels (no CS)",
        "SPI em dois canais analógicos (sem CS)",
    ),
    (
        "SPI as a two-channel oscilloscope sees it: CH1 is MOSI, CH2 is SCLK, 500 kHz, mode 0, no chip select. In Decode ▾, check CH1, then in its ▸ settings choose SPI and CH2 as SCLK: the words are found from the pauses of the clock.",
        "SPI como um osciloscópio de dois canais o vê: CH1 é MOSI, CH2 é SCLK, 500 kHz, modo 0, sem chip select. Em Decodificar ▾, marque CH1 e, nos seus ajustes ▸, escolha SPI e CH2 como SCLK: as palavras são separadas pelas pausas do clock.",
    ),
    (
        "Oscilloscope: RC filter step response",
        "Osciloscópio: resposta ao degrau de um filtro RC",
    ),
    (
        "Analog Discovery 3, Oscilloscope export (.csv): the Wavegen makes a 1 kHz square wave, 0 to 3.3 V (CH1), into an RC low-pass filter of 100 us (CH2). Try Measure ▾ for the rise time of CH2, or the time cursors at 63 % of the step.",
        "Analog Discovery 3, exportação do Oscilloscope (.csv): o Wavegen gera uma onda quadrada de 1 kHz, de 0 a 3,3 V (CH1), na entrada de um filtro passa-baixas RC de 100 µs (CH2). Experimente Medir ▾ para o tempo de subida do CH2, ou os cursores de tempo em 63 % do degrau.",
    ),
    (
        "Logic Analyzer: I2C sensor and UART",
        "Analisador lógico: sensor I2C e UART",
    ),
    (
        "Analog Discovery 3, Logic Analyzer export (.csv), 2 MHz: a microcontroller reads a temperature sensor at 0x48 on I2C (SCL, SDA, 100 kHz) and prints \"T=25.5C\" on TX (115200 baud). In Decode ▾, check SDA (I2C, SCL as clock) and TX (UART); Show ASCII to read the text.",
        "Analog Discovery 3, exportação do Logic Analyzer (.csv), 2 MHz: um microcontrolador lê um sensor de temperatura em 0x48 por I2C (SCL, SDA, 100 kHz) e escreve \"T=25.5C\" em TX (115200 baud). Em Decodificar ▾, marque SDA (I2C, SCL como clock) e TX (UART); Mostrar ASCII para ler o texto.",
    ),
    ("FNIRSI DPOX180H", "FNIRSI DPOX180H"),
    (
        "Captures saved on a DPOX180H by the author of this viewer",
        "Capturas salvas num DPOX180H pelo autor deste visualizador",
    ),
    ("FNIRSI 1013D", "FNIRSI 1013D"),
    (
        "Captures by lexus2026HA (github.com/lexus2026HA/FNIRSI-2C53T-Decoder), license CC BY-NC 4.0",
        "Capturas de lexus2026HA (github.com/lexus2026HA/FNIRSI-2C53T-Decoder), licença CC BY-NC 4.0",
    ),
    ("Rigol", "Rigol"),
    (
        "From RigolWFM, copyright (c) 2020-23 Scott Prahl (github.com/scottprahl/RigolWFM), license BSD 3-Clause",
        "Do RigolWFM, copyright (c) 2020-23 Scott Prahl (github.com/scottprahl/RigolWFM), licença BSD 3-Clause",
    ),
    ("Logic (VCD)", "Lógico (VCD)"),
    (
        "Made for this viewer by scripts/make-vcd-examples.py (not captures)",
        "Feitos para este visualizador por scripts/make-vcd-examples.py (não são capturas)",
    ),
    ("Digilent WaveForms", "Digilent WaveForms"),
    (
        "In the export format of Digilent WaveForms (Analog Discovery); made for this viewer by scripts/make-digilent-examples.py (not captures)",
        "No formato de exportação do Digilent WaveForms (Analog Discovery); feitos para este visualizador por scripts/make-digilent-examples.py (não são capturas)",
    ),
    ("PNG image", "Imagem PNG"),
    (
        "Could not save {file}: {e}.",
        "Não foi possível salvar {file}: {e}.",
    ),
    (
        "Could not save or print the picture: {e}.",
        "Não foi possível salvar ou imprimir a imagem: {e}.",
    ),
    (
        "Decode the lines checked:",
        "Decodificar as linhas marcadas:",
    ),
    (
        "A bus has values, not one line",
        "Um barramento tem valores, não é uma linha",
    ),
    ("Protocol:", "Protocolo:"),
    (
        "The protocol and settings of this line",
        "O protocolo e os ajustes desta linha",
    ),
    ("Show:", "Mostrar:"),
    (
        "Each line has its own settings (▸). The words are drawn just above the signal and listed under the plot; click a word in the list to show it.",
        "Cada linha tem seus próprios ajustes (▸). As palavras aparecem logo acima do sinal e numa lista embaixo do gráfico; clique numa palavra da lista para mostrá-la.",
    ),
    ("⎘ Copy", "⎘ Copiar"),
    ("SPI, choose SCLK", "SPI, escolha o SCLK"),
    ("I2C, choose SCL", "I2C, escolha o SCL"),
    ("Auto", "Auto"),
    ("Threshold:", "Limiar:"),
    (
        "The middle between the lowest and the highest sample",
        "O meio entre a menor e a maior amostra",
    ),
    ("Clock (SCLK):", "Clock (SCLK):"),
    ("Chip select (CS):", "Chip select (CS):"),
    ("None", "Nenhum"),
    (
        "Without CS, a pause of the clock starts a new word",
        "Sem CS, uma pausa do clock começa uma nova palavra",
    ),
    ("CS active:", "CS ativo:"),
    ("Low", "Baixo"),
    ("High", "Alto"),
    ("Mode:", "Modo:"),
    (
        "CPOL 0, CPHA 0: clock idle low, read on the rising edge",
        "CPOL 0, CPHA 0: clock em repouso baixo, leitura na borda de subida",
    ),
    (
        "CPOL 0, CPHA 1: clock idle low, read on the falling edge",
        "CPOL 0, CPHA 1: clock em repouso baixo, leitura na borda de descida",
    ),
    (
        "CPOL 1, CPHA 0: clock idle high, read on the falling edge",
        "CPOL 1, CPHA 0: clock em repouso alto, leitura na borda de descida",
    ),
    (
        "CPOL 1, CPHA 1: clock idle high, read on the rising edge",
        "CPOL 1, CPHA 1: clock em repouso alto, leitura na borda de subida",
    ),
    ("Bit order:", "Ordem dos bits:"),
    ("MSB first", "MSB primeiro"),
    ("LSB first", "LSB primeiro"),
    ("Word size:", "Tamanho da palavra:"),
    ("Baud:", "Baud:"),
    (
        "Measured from the shortest pulse",
        "Medido pelo pulso mais curto",
    ),
    ("Data bits:", "Bits de dados:"),
    ("Parity:", "Paridade:"),
    ("Stop bits:", "Bits de parada:"),
    ("Idle level:", "Nível de repouso:"),
    (
        "TTL serial of a microcontroller",
        "Serial TTL de um microcontrolador",
    ),
    ("Low (inverted)", "Baixo (invertido)"),
    (
        "RS-232 line levels (before the transceiver)",
        "Níveis de linha RS-232 (antes do transceptor)",
    ),
    ("Sr", "Sr"),
    ("start", "start"),
    ("repeated start", "start repetido"),
    ("stop", "stop"),
    (
        "CS active (start of a selection)",
        "CS ativo (início de uma seleção)",
    ),
    ("CS released", "CS liberado"),
    ("read", "leitura"),
    ("write", "escrita"),
    ("Decoder of {label}", "Decodificador de {label}"),
    (
        "{label} is SDA (data). Clock line (SCL):",
        "{label} é o SDA (dados). Linha de clock (SCL):",
    ),
    (", {errors} with errors", ", {errors} com erros"),
    (
        "Copy all the words of {label}, as shown",
        "Copiar todas as palavras de {label}, como aparecem",
    ),
    (
        "{label} #{a0} at {a1}\n{a2}\nClick to show it in the plot",
        "{label} nº {a0} em {a1}\n{a2}\nClique para mostrar no gráfico",
    ),
    (" … {a0} more", " … mais {a0}"),
    ("SPI mode {a0}, SCLK: {a1}", "SPI modo {a0}, SCLK: {a1}"),
    (
        "{label} is a data line (MOSI or MISO).",
        "{label} é uma linha de dados (MOSI ou MISO).",
    ),
    ("{a0} (hex) = {a1} = '{a2}'", "{a0} (hex) = {a1} = '{a2}'"),
    ("address {a0} (hex), {a1}", "endereço {a0} (hex), {a1}"),
    ("WaveGlance", "WaveGlance"),
    (
        "A quick look at oscilloscope and logic analyzer files: FNIRSI DPOX180H and 1013D/1014D (.wav), Keysight/Agilent (.bin), Rigol (.bin, .wfm), Digilent WaveForms (Analog Discovery), VCD, and CSV files from any oscilloscope. Measurements, cursors, FFT, Math, search, and UART, I2C and SPI decoders. The files are read on this device: nothing is uploaded.",
        "Uma olhada rápida em arquivos de osciloscópio e analisador lógico: FNIRSI DPOX180H e 1013D/1014D (.wav), Keysight/Agilent (.bin), Rigol (.bin, .wfm), Digilent WaveForms (Analog Discovery), VCD e arquivos CSV de qualquer osciloscópio. Medições, cursores, FFT, Math, busca e decodificadores UART, I2C e SPI. Os arquivos são lidos neste dispositivo: nada é enviado.",
    ),
    ("Try an example", "Experimente um exemplo"),
    (
        "Click one to open it. File ▾ → Open example has them too.",
        "Clique em um para abrir. Também estão em Arquivo ▾ → Abrir exemplo.",
    ),
    ("Controls", "Controles"),
    ("Don't show this at startup", "Não mostrar ao iniciar"),
    ("⏏ Open file…", "⏏ Abrir arquivo…"),
    ("? Help", "? Ajuda"),
    ("Close", "Fechar"),
    ("Help", "Ajuda"),
    (
        "☰ Welcome screen and examples",
        "☰ Tela de boas-vindas e exemplos",
    ),
    (
        "The welcome screen, with the built-in examples",
        "A tela de boas-vindas, com os exemplos incluídos",
    ),
    ("Close this message", "Fechar esta mensagem"),
    ("Open as…", "Abrir como…"),
    (
        "Choose the format of this file yourself",
        "Escolher você mesmo o formato deste arquivo",
    ),
    (
        "Choose the format to read this file with.",
        "Escolha o formato para ler este arquivo.",
    ),
    (
        "Which format is this file?",
        "Qual é o formato deste arquivo?",
    ),
    ("Possible formats", "Formatos possíveis"),
    ("certain", "certo"),
    ("likely", "provável"),
    ("possible", "possível"),
    ("Other formats", "Outros formatos"),
    ("Cancel", "Cancelar"),
    ("Print", "Imprimir"),
    (
        "White background (recommended)",
        "Fundo branco (recomendado)",
    ),
    (
        "Uses much less ink or toner, and the traces use darker colours that stay easy to read on paper.",
        "Gasta muito menos tinta ou toner, e os traços usam cores mais escuras, fáceis de ler no papel.",
    ),
    ("Black background", "Fundo preto"),
    (
        "Like the scope screen. Prints a lot of black: much more ink or toner.",
        "Como a tela do osciloscópio. Imprime muito preto: bem mais tinta ou toner.",
    ),
    (
        "The screen changes only for the picture; it comes back by itself.",
        "A tela muda só para a imagem; depois volta sozinha.",
    ),
    ("Print…", "Imprimir…"),
    ("Statistics per cycle", "Estatísticas por ciclo"),
    (
        "Tick a time measurement (Freq, Period, Rise, Fall, ±Width, Duty) in the Measure menu.",
        "Marque uma medição de tempo (Freq, Período, Subida, Descida, ±Largura, Ciclo) no menu Medir.",
    ),
    ("Open a file", "Abra um arquivo"),
    (
        "Drop a file on the window, or use ⏏ File ▾ → Open file….",
        "Solte um arquivo na janela, ou use ⏏ Arquivo ▾ → Abrir arquivo….",
    ),
    (
        "File ▾ → Open example, or the welcome screen (button at the top of this window): real FNIRSI DPOX180H, 1013D and Rigol captures, VCD files (a CPU bus, a DAC, UART, I2C and SPI) and Digilent WaveForms exports, to try the viewer without a scope.",
        "Arquivo ▾ → Abrir exemplo, ou a tela de boas-vindas (botão no alto desta janela): capturas reais de FNIRSI DPOX180H, 1013D e Rigol, arquivos VCD (um barramento de CPU, um DAC, UART, I2C e SPI) e exportações do Digilent WaveForms, para experimentar o visualizador sem osciloscópio.",
    ),
    (
        "Supported: FNIRSI DPOX180H, FNIRSI 1013D/1014D (original firmware), Keysight/Agilent .bin, Rigol .bin and .wfm (DS1000Z, DS1000E/D, DS2000), Digilent WaveForms exports (Analog Discovery: Oscilloscope and Logic Analyzer, .csv and .txt), VCD (logic analyzers: digital signals and buses in lanes at the bottom), and CSV/text tables from any oscilloscope (Keysight, Rigol, Tektronix, Siglent… and this viewer's own export). The format is found automatically; the maker and model are shown at the top left.",
        "Suportados: FNIRSI DPOX180H, FNIRSI 1013D/1014D (firmware original), Keysight/Agilent .bin, Rigol .bin e .wfm (DS1000Z, DS1000E/D, DS2000), exportações do Digilent WaveForms (Analog Discovery: Oscilloscope e Logic Analyzer, .csv e .txt), VCD (analisadores lógicos: sinais digitais e barramentos em faixas embaixo) e tabelas CSV/texto de qualquer osciloscópio (Keysight, Rigol, Tektronix, Siglent… e a própria exportação deste visualizador). O formato é detectado automaticamente; o fabricante e o modelo aparecem no alto, à esquerda.",
    ),
    (
        "If the format is not certain, a window asks which one it is. If a file cannot be read, the red banner has an \"Open as…\" button to choose the format yourself.",
        "Se o formato não for certo, uma janela pergunta qual é. Se um arquivo não puder ser lido, a faixa vermelha tem um botão \"Abrir como…\" para você escolher o formato.",
    ),
    (
        "CSV: the first number column is the time (s), the others are channels; the names come from the header line. Files with up to millions of samples are drawn fast: only the part in view, at about two points per pixel.",
        "CSV: a primeira coluna numérica é o tempo (s), as outras são canais; os nomes vêm da linha de cabeçalho. Arquivos com até milhões de amostras são desenhados rápido: só a parte visível, com cerca de dois pontos por pixel.",
    ),
    (
        "The file is read on this device; nothing is uploaded.",
        "O arquivo é lido neste dispositivo; nada é enviado.",
    ),
    (
        "If a file cannot be opened, a red banner says why, and the file shown before stays.",
        "Se um arquivo não puder ser aberto, uma faixa vermelha diz o motivo, e o arquivo mostrado antes continua na tela.",
    ),
    (
        "Compare, export and print (⏏ File menu)",
        "Comparar, exportar e imprimir (menu ⏏ Arquivo)",
    ),
    (
        "Open reference…: a second file drawn faint behind the traces (grey for CH1, blue-grey for CH2), to compare before and after. Click the REF readout at the bottom to hide or show it.",
        "Abrir referência…: um segundo arquivo desenhado em tom claro atrás dos traços (cinza para o CH1, cinza-azulado para o CH2), para comparar antes e depois. Clique na leitura REF embaixo para esconder ou mostrar.",
    ),
    (
        "Export CSV…: all the samples as a table (time and the voltage of each channel), with the settings in # lines at the top.",
        "Exportar CSV…: todas as amostras numa tabela (tempo e a tensão de cada canal), com os ajustes em linhas # no início.",
    ),
    (
        "Save screenshot (PNG)…: a picture of the screen as shown.",
        "Salvar imagem da tela (PNG)…: uma imagem da tela como ela está.",
    ),
    (
        "Print… (Ctrl/⌘+P): choose a white background (recommended: much less ink, darker colours made for paper) or black; the screen changes only for the picture. On the desktop, the picture opens in the image viewer, where you can print it.",
        "Imprimir… (Ctrl/⌘+P): escolha fundo branco (recomendado: muito menos tinta, cores mais escuras feitas para o papel) ou preto; a tela muda só para a imagem. No desktop, a imagem abre no visualizador de imagens, onde você pode imprimi-la.",
    ),
    ("Move and zoom", "Mover e dar zoom"),
    (
        "Mouse wheel on the plot: zoom. Shift + wheel: time only. Alt + wheel: volts only.",
        "Roda do mouse no gráfico: zoom. Shift + roda: só o tempo. Alt + roda: só a tensão.",
    ),
    (
        "Mouse wheel on the time or voltage axis: move along that axis.",
        "Roda do mouse no eixo do tempo ou da tensão: move ao longo desse eixo.",
    ),
    (
        "Trackpad: a vertical swipe zooms, a horizontal swipe moves along the time axis.",
        "Trackpad: deslizar na vertical dá zoom, deslizar na horizontal move ao longo do tempo.",
    ),
    (
        "Drag: move. Double-click: back to the start view.",
        "Arrastar: move. Clique duplo: volta à vista inicial.",
    ),
    (
        "Touch: pinch to zoom, drag to move, double-tap to reset.",
        "Toque: pinça para zoom, arrastar para mover, toque duplo para restaurar.",
    ),
    (
        "⊕ Zoom: an overview of the whole record above the plot. The bright window is the part shown below. Click or drag in it to move there; wheel on it to zoom in time.",
        "⊕ Zoom: uma visão geral de todo o registro acima do gráfico. A janela clara é a parte mostrada embaixo. Clique ou arraste nela para ir até lá; roda sobre ela para zoom no tempo.",
    ),
    (
        "⬌ Fit H: whole record. ⬍ Fit V: vertical axis fitted to what is on screen now. ⛶ Fit both: both. Scope screen: the time window the scope showed (a file opens on it when it stores it).",
        "⬌ Ajustar H: todo o registro. ⬍ Ajustar V: eixo vertical ajustado ao que está na tela agora. ⛶ Ajustar ambos: os dois. Tela do osciloscópio: a janela de tempo que o osciloscópio mostrava (o arquivo abre nela, quando a guarda).",
    ),
    (
        "Logic signals only (VCD, Digilent Logic Analyzer): the axis is \"Logic\"; Y, Math, voltage cursors and voltage measurements are off; the FFT shows frequencies, with levels in dB relative to the peak.",
        "Só sinais lógicos (VCD, Logic Analyzer do Digilent): o eixo é \"Lógico\"; Y, Math, cursores de tensão e medições de tensão ficam desligados; a FFT mostra as frequências, com níveis em dB relativos ao pico.",
    ),
    (
        "Buses (VCD, Digilent Logic Analyzer): the value of each part is written in its lane. Right-click the bus readout at the bottom: Hex, Dec, Bin or ASCII. Hover a bus lane to read the value at the pointer.",
        "Barramentos (VCD, Logic Analyzer do Digilent): o valor de cada trecho aparece escrito na sua faixa. Clique com o botão direito na leitura do barramento embaixo: Hex, Dec, Bin ou ASCII. Passe o ponteiro sobre a faixa para ler o valor naquele ponto.",
    ),
    (
        "Vertical axis (Y: Volts / Div)",
        "Eixo vertical (Y: Volts / Div)",
    ),
    (
        "Volts: one voltage axis for both channels.",
        "Volts: um eixo de tensão para os dois canais.",
    ),
    (
        "Div: like the scope screen, 8 divisions. Each channel uses its own V/div and starts from its ground marker.",
        "Div: como a tela do osciloscópio, 8 divisões. Cada canal usa o seu V/div e parte do seu marcador de terra.",
    ),
    (
        "1▶ and 2▶ on the left edge: the 0 V level (ground) of each channel. Drag a marker to move that channel up or down. This needs Y: Div (each channel has its own scale); a drag in Volts mode switches to Div.",
        "1▶ e 2▶ na borda esquerda: o nível de 0 V (terra) de cada canal. Arraste um marcador para subir ou descer aquele canal. Isso exige Y: Div (cada canal tem a sua escala); arrastar no modo Volts muda para Div.",
    ),
    (
        "Mouse wheel on the CH1 or CH2 readout (bottom bar): the V/div of that channel, in 1-2-5 steps, like the V/div knob (switches to Div). A * marks a V/div that is not the one in the file. Y ▾ → Reset channel positions and scales puts everything back.",
        "Roda do mouse na leitura do CH1 ou CH2 (barra de baixo): o V/div daquele canal, em passos 1-2-5, como o botão de V/div (muda para Div). Um * marca um V/div diferente do arquivo. Y ▾ → Restaurar posições e escalas dos canais põe tudo de volta.",
    ),
    (
        "The orange T on the top edge is the trigger (t = 0).",
        "O T laranja na borda de cima é o gatilho (t = 0).",
    ),
    ("Channels", "Canais"),
    (
        "Click the CH1 or CH2 readout at the bottom to hide or show that channel.",
        "Clique na leitura do CH1 ou CH2 embaixo para esconder ou mostrar aquele canal.",
    ),
    (
        "The readout shows V/div, probe (1X, 10X) and, if the file has it, the coupling (DC/AC).",
        "A leitura mostra o V/div, a ponta de prova (1X, 10X) e, se o arquivo tiver, o acoplamento (DC/AC).",
    ),
    ("Cursors (⌖ Cursors menu)", "Cursores (menu ⌖ Cursores)"),
    (
        "Time: two vertical lines T1, T2. Readout: T1, T2, ΔT, 1/ΔT, and each channel's voltage at T1 and T2.",
        "Tempo: duas linhas verticais T1, T2. Leitura: T1, T2, ΔT, 1/ΔT e a tensão de cada canal em T1 e T2.",
    ),
    (
        "Volt: two horizontal lines V1, V2. Readout: V1, V2, ΔV (in Div mode, in the volts of each channel).",
        "Tensão: duas linhas horizontais V1, V2. Leitura: V1, V2, ΔV (no modo Div, nos volts de cada canal).",
    ),
    (
        "Drag a dashed line to move it. Bring to view moves lost cursors back to the screen.",
        "Arraste uma linha tracejada para movê-la. Trazer para a vista devolve à tela os cursores perdidos.",
    ),
    ("Spectrum (∿ FFT menu)", "Espectro (menu ∿ FFT)"),
    (
        "Show FFT: the spectrum of one channel under the time plot, of the samples shown in the time plot (zoom in to analyse a part).",
        "Mostrar FFT: o espectro de um canal embaixo do gráfico do tempo, das amostras mostradas no gráfico do tempo (dê zoom para analisar uma parte).",
    ),
    (
        "Window: Hanning for general use; Rectangular for transients; Blackman-Harris for small harmonics; Flat top for accurate levels.",
        "Janela: Hanning para uso geral; Retangular para transitórios; Blackman-Harris para harmônicas pequenas; Flat top para níveis precisos.",
    ),
    (
        "Scale: dBV (0 dBV = 1 V RMS) or V RMS. A sine of amplitude A reads A/√2 V RMS.",
        "Escala: dBV (0 dBV = 1 V RMS) ou V RMS. Uma senoide de amplitude A mostra A/√2 V RMS.",
    ),
    (
        "Draw: Line, or Bars (one bar per frequency bin, like a spectrum analyzer). Zoomed out, the bins in one screen column make one bar, at the highest level.",
        "Desenho: Linha, ou Barras (uma barra por faixa de frequência, como um analisador de espectro). Com zoom afastado, as faixas de uma coluna da tela viram uma barra, no nível mais alto.",
    ),
    (
        "Peak: the highest line above DC (white dot); its frequency is interpolated between bins. Δf: the frequency step; a longer time span gives a finer step.",
        "Pico: a linha mais alta acima do DC (ponto branco); a frequência é interpolada entre as faixas. Δf: o passo de frequência; um intervalo de tempo maior dá um passo mais fino.",
    ),
    (
        "The FFT plot opens up to 20 × the peak frequency; ⛶ Fit (next to the readouts) or a double-click returns there; zoom out to see up to half the sample rate.",
        "O gráfico da FFT abre até 20 × a frequência do pico; ⛶ Ajustar (ao lado das leituras) ou um clique duplo volta para lá; afaste o zoom para ver até metade da taxa de amostragem.",
    ),
    (
        "Protocol decoder (⇄ Decode menu)",
        "Decodificador de protocolo (menu ⇄ Decodificar)",
    ),
    (
        "UART: check each line to decode (a channel or a VCD signal); several lines can be decoded at the same time. The ▸ next to a line opens its own settings: Baud (Auto measures the rate from the shortest pulse), data bits, parity, stop bits and the idle level as the device sends them.",
        "UART: marque cada linha a decodificar (um canal ou um sinal VCD); várias linhas podem ser decodificadas ao mesmo tempo. O ▸ ao lado de uma linha abre os seus ajustes: Baud (Auto mede a taxa pelo pulso mais curto), bits de dados, paridade, bits de parada e o nível de repouso, como o dispositivo envia.",
    ),
    (
        "An analog channel is made into 0 and 1 with a threshold (in its ▸ settings): Auto (the middle of the signal, with hysteresis) or a value in volts.",
        "Um canal analógico vira 0 e 1 com um limiar (nos seus ajustes ▸): Auto (o meio do sinal, com histerese) ou um valor em volts.",
    ),
    (
        "The words are drawn just above the decoded signal and listed in a row under the plot: Hex, Dec, Bin or ASCII. Click a word in the row to centre it; ⎘ Copy copies all of them. A word with a parity or framing error is in the warning colour.",
        "As palavras aparecem logo acima do sinal decodificado e numa fileira embaixo do gráfico: Hex, Dec, Bin ou ASCII. Clique numa palavra da fileira para centralizá-la; ⎘ Copiar copia todas. Uma palavra com erro de paridade ou de enquadramento aparece na cor de alerta.",
    ),
    (
        "I2C: check the SDA line, and in its ▸ settings choose I2C and the SCL line (a line named sda gets I2C with scl by itself). S = start, Sr = repeated start, W/R = address and direction, N = not acknowledged (NACK), P = stop; green and red dots mark start and stop.",
        "I2C: marque a linha SDA e, nos seus ajustes ▸, escolha I2C e a linha SCL (uma linha chamada sda recebe I2C com scl automaticamente). S = start, Sr = start repetido, W/R = endereço e direção, N = sem confirmação (NACK), P = stop; pontos verdes e vermelhos marcam start e stop.",
    ),
    (
        "SPI: check the data line (MOSI or MISO, or both), and in its ▸ settings choose SPI, the SCLK line, the CS line (or None), the mode (0–3), the bit order and the word size. Lines named mosi/miso get SPI with sck and cs by themselves. [ and ] in the row: CS active and released. Without CS, a pause of the clock starts a new word.",
        "SPI: marque a linha de dados (MOSI ou MISO, ou as duas) e, nos seus ajustes ▸, escolha SPI, a linha SCLK, a linha CS (ou Nenhuma), o modo (0–3), a ordem dos bits e o tamanho da palavra. Linhas chamadas mosi/miso recebem SPI com sck e cs automaticamente. [ e ] na fileira: CS ativo e liberado. Sem CS, uma pausa do clock começa uma nova palavra.",
    ),
    (
        "Use a time base that gives at least 4 samples per bit (10 or more is better); the row warns when there are fewer.",
        "Use uma base de tempo com pelo menos 4 amostras por bit (10 ou mais é melhor); a fileira avisa quando há menos.",
    ),
    ("Measurements (▤ Measure menu)", "Medições (menu ▤ Medir)"),
    (
        "Tick the measurements and channels to show. The choice is saved.",
        "Marque as medições e os canais a mostrar. A escolha fica salva.",
    ),
    (
        "Samples: Screen (the time span shown), Record (the whole file) or T1–T2 (between the time cursors).",
        "Amostras: Tela (o intervalo mostrado), Registro (o arquivo todo) ou T1–T2 (entre os cursores de tempo).",
    ),
    (
        "Values have 4 significant digits and never more resolution than the samples.",
        "Os valores têm 4 algarismos significativos e nunca mais resolução que as amostras.",
    ),
    (
        "?1.91 V in orange: the signal is clipped (it reaches the limit of the input range); the real value can be larger.",
        "?1,91 V em laranja: o sinal está saturado (chega ao limite da faixa de entrada); o valor real pode ser maior.",
    ),
    (
        "<4.00 µs: the edge is faster than 2 samples; only an upper limit is known.",
        "<4,00 µs: a borda é mais rápida que 2 amostras; só se conhece um limite superior.",
    ),
    (
        "—: the value cannot be measured (for example, fewer than two edges for a frequency).",
        "—: o valor não pode ser medido (por exemplo, menos de duas bordas para uma frequência).",
    ),
    ("Math and search", "Math e busca"),
    (
        "Σ Math: CH1 + CH2, CH1 − CH2 or CH2 − CH1, sample by sample (needs both channels). The MATH trace is measured like a channel; click the MATH readout to hide it.",
        "Σ Math: CH1 + CH2, CH1 − CH2 ou CH2 − CH1, amostra por amostra (precisa dos dois canais). O traço MATH é medido como um canal; clique na leitura MATH para escondê-lo.",
    ),
    (
        "⌕ Search: rising edges, falling edges or glitches (very short pulses) in the whole record of CH1 or CH2. Marks on the bottom edge show them; Previous/Next (keys [ and ]) centre the view on each one.",
        "⌕ Buscar: bordas de subida, bordas de descida ou glitches (pulsos muito curtos) em todo o registro do CH1 ou CH2. Marcas na borda de baixo mostram onde estão; Anterior/Próximo (teclas [ e ]) centralizam a vista em cada um.",
    ),
    (
        "Statistics per cycle (▤ Measure menu): mean, min, max, σ and number of cycles for the time measurements.",
        "Estatísticas por ciclo (menu ▤ Medir): média, mín., máx., σ e número de ciclos das medições de tempo.",
    ),
    ("Keyboard", "Teclado"),
    (
        "Ctrl/⌘+O: open a file. Ctrl/⌘+P: print. F1: this help. B: black or white screen. [ and ]: previous / next search result.",
        "Ctrl/⌘+O: abrir um arquivo. Ctrl/⌘+P: imprimir. F1: esta ajuda. B: tela preta ou branca. [ e ]: resultado anterior / próximo da busca.",
    ),
    (
        "F: fit both. H: fit time. V: fit vertical.",
        "F: ajustar ambos. H: ajustar o tempo. V: ajustar a vertical.",
    ),
    (
        "Z: zoom view. S: spectrum (FFT). Y: volts or divisions. 1, 2: hide or show CH1, CH2.",
        "Z: visão de zoom. S: espectro (FFT). Y: volts ou divisões. 1, 2: esconder ou mostrar CH1, CH2.",
    ),
    (
        "+ and −: zoom the time axis. Arrows: move the view (Shift: more).",
        "+ e −: zoom no eixo do tempo. Setas: movem a vista (Shift: mais).",
    ),
    (
        "C: cursor mode (off, time, volt, both). Tab: select the next cursor; then the arrows move it (1 sample or 1/100 of the screen; Shift: 1/10). Esc: unselect.",
        "C: modo dos cursores (desligado, tempo, tensão, ambos). Tab: seleciona o próximo cursor; então as setas o movem (1 amostra ou 1/100 da tela; Shift: 1/10). Esc: tira a seleção.",
    ),
    (
        "Esc: close this window, the welcome screen or an error message.",
        "Esc: fecha esta janela, a tela de boas-vindas ou uma mensagem de erro.",
    ),
    ("Mouse wheel", "Roda do mouse"),
    (
        "Zoom (Shift: time only, Alt: volts only)",
        "Zoom (Shift: só o tempo, Alt: só a tensão)",
    ),
    ("Wheel on an axis", "Roda num eixo"),
    ("Pan along that axis", "Mover ao longo desse eixo"),
    ("Drag", "Arrastar"),
    ("Pan", "Mover"),
    ("Double-click", "Clique duplo"),
    ("Reset the view", "Restaurar a vista"),
    ("Drag a dashed line", "Arrastar uma linha tracejada"),
    (
        "Move a cursor (⌖ Cursors menu)",
        "Mover um cursor (menu ⌖ Cursores)",
    ),
    ("Drag 1▶ / 2▶", "Arrastar 1▶ / 2▶"),
    (
        "Move that channel up or down (switches Y to Div)",
        "Subir ou descer aquele canal (muda Y para Div)",
    ),
    ("Click CH1 / CH2 readout", "Clicar na leitura CH1 / CH2"),
    ("Hide or show the channel", "Esconder ou mostrar o canal"),
    ("Touch", "Toque"),
    (
        "Pinch: zoom · drag: pan · double-tap: reset",
        "Pinça: zoom · arrastar: mover · toque duplo: restaurar",
    ),
    ("Keys", "Teclas"),
    (
        "F fit · Z zoom view · S FFT · C cursors · Tab + arrows move a cursor",
        "F ajustar · Z visão de zoom · S FFT · C cursores · Tab + setas movem um cursor",
    ),
    ("Esc", "Esc"),
    (
        "Close this screen or an error message",
        "Fechar esta tela ou uma mensagem de erro",
    ),
    (
        "v{VERSION} · commit {GIT_HASH}",
        "v{VERSION} · commit {GIT_HASH}",
    ),
    (
        "Each cycle in the {window_name} window: mean, min, max, standard deviation σ and number n.",
        "Cada ciclo na janela {window_name}: média, mín., máx., desvio padrão σ e número n.",
    ),
    (
        "WaveGlance {VERSION} · commit {GIT_HASH}",
        "WaveGlance {VERSION} · commit {GIT_HASH}",
    ),
    ("{a0}\n\nLink: {a1}", "{a0}\n\nLink: {a1}"),
    ("dropped file", "arquivo solto"),
    (
        "Waveform (.wav, .bin, .wfm, .vcd, .csv, .txt)",
        "Forma de onda (.wav, .bin, .wfm, .vcd, .csv, .txt)",
    ),
    ("All files", "Todos os arquivos"),
    (
        "Needs two channels in the file.",
        "Precisa de dois canais no arquivo.",
    ),
    ("Off", "Desligado"),
    (
        "Sample by sample, in volts, at the times of A (B is interpolated when its samples are at other times). Measured like a channel (MATH row).",
        "Amostra por amostra, em volts, nos instantes de A (B é interpolado quando as suas amostras estão em outros instantes). Medido como um canal (linha MATH).",
    ),
    (
        "Pulses much shorter than the others (shorter than 10% of the typical time between edges)",
        "Pulsos muito mais curtos que os outros (mais curtos que 10% do tempo típico entre bordas)",
    ),
    (
        "At the 50% level between Vbase and Vtop, with hysteresis (10%–90%)",
        "No nível de 50% entre Vbase e Vtop, com histerese (10%–90%)",
    ),
    ("In:", "Em:"),
    ("◀ Previous", "◀ Anterior"),
    ("Key [", "Tecla ["),
    ("Next ▶", "Próximo ▶"),
    ("Key ]", "Tecla ]"),
    ("Volts", "Volts"),
    (
        "Vertical axis in volts: both channels share the axis",
        "Eixo vertical em volts: os dois canais compartilham o eixo",
    ),
    ("Div", "Div"),
    (
        "Vertical axis in divisions, like the scope screen: each channel has its own V/div and ground marker",
        "Eixo vertical em divisões, como a tela do osciloscópio: cada canal tem o seu V/div e o seu marcador de terra",
    ),
    (
        "Reset channel positions and scales",
        "Restaurar posições e escalas dos canais",
    ),
    (
        "Put the ground markers and the V/div back as the scope had them",
        "Devolver os marcadores de terra e o V/div ao que o osciloscópio tinha",
    ),
    ("Open file…", "Abrir arquivo…"),
    (
        "Open a file (Ctrl/⌘+O), or drop it on the window",
        "Abrir um arquivo (Ctrl/⌘+O), ou soltá-lo na janela",
    ),
    ("Open example", "Abrir exemplo"),
    ("Open reference…", "Abrir referência…"),
    (
        "Open a second file, drawn faint behind the traces, to compare with",
        "Abrir um segundo arquivo, desenhado em tom claro atrás dos traços, para comparar",
    ),
    ("Close reference", "Fechar referência"),
    ("Export CSV…", "Exportar CSV…"),
    (
        "Save the samples as a table: time and the voltage of each channel",
        "Salvar as amostras numa tabela: o tempo e a tensão de cada canal",
    ),
    ("Save screenshot (PNG)…", "Salvar imagem da tela (PNG)…"),
    (
        "Save a picture of the plots and readouts, as shown",
        "Salvar uma imagem dos gráficos e leituras, como aparecem",
    ),
    (
        "Print the plots and readouts on a white screen (Ctrl/⌘+P)",
        "Imprimir os gráficos e leituras com a tela branca (Ctrl/⌘+P)",
    ),
    (
        "Reads FNIRSI DPOX180H and 1013D/1014D .wav, Keysight/Agilent .bin, Rigol .bin and .wfm, Digilent WaveForms .csv and .txt, VCD, and CSV files of any oscilloscope",
        "Lê .wav de FNIRSI DPOX180H e 1013D/1014D, .bin de Keysight/Agilent, .bin e .wfm de Rigol, .csv e .txt do Digilent WaveForms, VCD e arquivos CSV de qualquer osciloscópio",
    ),
    ("⏏ File ▾", "⏏ Arquivo ▾"),
    (
        "Open, examples, reference, export, print",
        "Abrir, exemplos, referência, exportar, imprimir",
    ),
    ("⛶ Fit ▾", "⛶ Ajustar ▾"),
    (
        "Fit the view to the waveform",
        "Ajustar a vista à forma de onda",
    ),
    ("Y: Volts ▾", "Y: Volts ▾"),
    ("Y: Div ▾", "Y: Div ▾"),
    ("Y: Logic", "Y: Lógico"),
    (
        "the lanes have no voltage axis",
        "as faixas não têm eixo de tensão",
    ),
    (
        "Vertical axis: volts, or divisions like the scope screen",
        "Eixo vertical: volts, ou divisões como a tela do osciloscópio",
    ),
    ("⊕ Zoom", "⊕ Zoom"),
    (
        "Zoom view: an overview of the whole record above the plot. Click or drag in it to move, wheel to zoom.",
        "Visão de zoom: uma visão geral de todo o registro acima do gráfico. Clique ou arraste nela para mover, roda para zoom.",
    ),
    (
        "Spectrum of a channel (FFT) under the plot",
        "Espectro de um canal (FFT) embaixo do gráfico",
    ),
    ("⌖ Cursors ▾", "⌖ Cursores ▾"),
    ("Measurement cursors", "Cursores de medição"),
    ("▤ Measure ▾", "▤ Medir ▾"),
    (
        "Choose the automatic measurements to show",
        "Escolher as medições automáticas a mostrar",
    ),
    ("Σ Math ▾", "Σ Math ▾"),
    (
        "the Math operations work on voltages",
        "as operações de Math trabalham com tensões",
    ),
    (
        "A channel computed from CH1 and CH2",
        "Um canal calculado a partir do CH1 e do CH2",
    ),
    ("⌕ Search ▾", "⌕ Buscar ▾"),
    (
        "Find edges or glitches in the whole record",
        "Encontrar bordas ou glitches em todo o registro",
    ),
    ("⇄ Decode ▾", "⇄ Decodificar ▾"),
    (
        "Protocol decoder (logic analyzer): serial data as bytes",
        "Decodificador de protocolo (analisador lógico): dados seriais como bytes",
    ),
    ("◐ Black", "◐ Preto"),
    (
        "Black screen, like the scope",
        "Tela preta, como no osciloscópio",
    ),
    ("◐ White", "◐ Branco"),
    (
        "White screen with darker traces, for printing",
        "Tela branca com traços mais escuros, para imprimir",
    ),
    ("Language of the interface", "Idioma da interface"),
    (
        "How to use the viewer: controls, cursors, measurements",
        "Como usar o visualizador: controles, cursores, medições",
    ),
    ("⬌ Fit H", "⬌ Ajustar H"),
    ("Show the whole record", "Mostrar todo o registro"),
    ("⬍ Fit V", "⬍ Ajustar V"),
    (
        "Fit the voltage axis to the samples between the first and last instant shown",
        "Ajustar o eixo de tensão às amostras entre o primeiro e o último instante mostrados",
    ),
    ("⛶ Fit both", "⛶ Ajustar ambos"),
    (
        "Full time span, then fit the voltage axis",
        "Todo o intervalo de tempo, depois ajustar o eixo de tensão",
    ),
    ("Scope screen", "Tela do osciloscópio"),
    (
        "Show the time window the scope showed on its screen (the file opens on it). Fit H shows the whole record.",
        "Mostrar a janela de tempo que o osciloscópio mostrava na tela (o arquivo abre nela). Ajustar H mostra todo o registro.",
    ),
    (
        "This file does not store the screen window",
        "Este arquivo não guarda a janela da tela",
    ),
    ("No cursors", "Sem cursores"),
    (
        "Time cursors T1 and T2 (vertical lines)",
        "Cursores de tempo T1 e T2 (linhas verticais)",
    ),
    (
        "Voltage cursors V1 and V2 (horizontal lines)",
        "Cursores de tensão V1 e V2 (linhas horizontais)",
    ),
    ("Time and voltage cursors", "Cursores de tempo e de tensão"),
    (
        "Only logic signals in this file: voltage cursors measure nothing",
        "Só sinais lógicos neste arquivo: cursores de tensão não medem nada",
    ),
    ("Bring to view", "Trazer para a vista"),
    (
        "Move the cursors to 1/4 and 3/4 of the screen",
        "Mover os cursores para 1/4 e 3/4 da tela",
    ),
    (
        "Drag a dashed cursor line to move it.",
        "Arraste uma linha tracejada de cursor para movê-la.",
    ),
    ("Channels:", "Canais:"),
    (
        "A window with mean, min, max, σ and number of cycles of the time measurements",
        "Uma janela com média, mín., máx., σ e número de ciclos das medições de tempo",
    ),
    ("Samples:", "Amostras:"),
    (
        "The time span shown now",
        "O intervalo de tempo mostrado agora",
    ),
    (
        "All the samples in the file",
        "Todas as amostras do arquivo",
    ),
    (
        "Between the time cursors (the screen if they are off)",
        "Entre os cursores de tempo (a tela, se estiverem desligados)",
    ),
    ("the steps are ideal", "os degraus são ideais"),
    ("All", "Todas"),
    ("Default", "Padrão"),
    (
        "Freq and Vpp, both channels, screen samples",
        "Freq e Vpp, os dois canais, amostras da tela",
    ),
    ("Show FFT", "Mostrar FFT"),
    ("Source:", "Fonte:"),
    ("Window:", "Janela:"),
    ("Scale:", "Escala:"),
    ("dBV", "dBV"),
    (
        "Decibels relative to 1 V RMS",
        "Decibéis relativos a 1 V RMS",
    ),
    ("Draw:", "Desenho:"),
    ("Line", "Linha"),
    (
        "A line through the levels of the frequency bins",
        "Uma linha pelos níveis das faixas de frequência",
    ),
    ("Bars", "Barras"),
    (
        "One bar per frequency bin, like a spectrum analyzer. Zoomed out, the bins in one screen column make one bar (the highest level)",
        "Uma barra por faixa de frequência, como um analisador de espectro. Com zoom afastado, as faixas de uma coluna da tela viram uma barra (o nível mais alto)",
    ),
    (
        "The FFT uses the samples shown in the time plot.",
        "A FFT usa as amostras mostradas no gráfico do tempo.",
    ),
    ("Time", "Tempo"),
    ("Volt", "Tensão"),
    ("Both", "Ambos"),
    (
        "{a0} found in the whole record",
        "{a0} encontrados em todo o registro",
    ),
    ("Show reference ({name})", "Mostrar referência ({name})"),
    (
        "WaveGlance {VERSION}, commit {GIT_HASH}",
        "WaveGlance {VERSION}, commit {GIT_HASH}",
    ),
    ("⌖ Cursors: {a0} ▾", "⌖ Cursores: {a0} ▾"),
    ("Σ Math: {a0} ▾", "Σ Math: {a0} ▾"),
    ("⌕ Search: {a0} ▾", "⌕ Buscar: {a0} ▾"),
    ("⇄ Decode: ×{n} ▾", "⇄ Decodificar: ×{n} ▾"),
    (
        "{a0}\nNot for logic signals: {a1}",
        "{a0}\nNão serve para sinais lógicos: {a1}",
    ),
    ("{tip} (open a file first)", "{tip} (abra um arquivo antes)"),
    (
        "Only logic signals in this file: {why}",
        "Só sinais lógicos neste arquivo: {why}",
    ),
    (
        "No known format was recognised in this file.",
        "Nenhum formato conhecido foi reconhecido neste arquivo.",
    ),
    (
        "The format of this file is not certain.",
        "O formato deste arquivo não é certo.",
    ),
    ("reference ", "referência "),
    (" Still showing {shown}.", " Continua mostrando {shown}."),
    (
        "Could not open {name}: {error}.{still}",
        "Não foi possível abrir {name}: {error}.{still}",
    ),
    (
        "internal error in the reader ({detail}). The file may be damaged, or of a kind this viewer does not know. Please report it at {ISSUES}",
        "erro interno no leitor ({detail}). O arquivo pode estar danificado, ou ser de um tipo que este visualizador não conhece. Por favor, relate em {ISSUES}",
    ),
    (
        "FFT: not enough samples in the time span shown",
        "FFT: amostras insuficientes no intervalo de tempo mostrado",
    ),
    (
        "FFT: the signal does not change in the time span shown",
        "FFT: o sinal não varia no intervalo de tempo mostrado",
    ),
    ("dB", "dB"),
    ("Peak", "Pico"),
    ("Hz", "Hz"),
    (
        "Frequency step between the FFT bins",
        "Passo de frequência entre as faixas da FFT",
    ),
    (
        "Samples used: the time span shown, sampled evenly",
        "Amostras usadas: o intervalo de tempo mostrado, amostrado de modo uniforme",
    ),
    (
        "Samples used (the time span shown)",
        "Amostras usadas (o intervalo de tempo mostrado)",
    ),
    ("⛶ Fit", "⛶ Ajustar"),
    (
        "Show the spectrum up to 20 × the peak frequency, with all levels (the start view). Double-click on the FFT plot does the same.",
        "Mostrar o espectro até 20 × a frequência do pico, com todos os níveis (a vista inicial). Um clique duplo no gráfico da FFT faz o mesmo.",
    ),
    ("Frequency", "Frequência"),
    ("dB (rel. to peak)", "dB (rel. ao pico)"),
    ("Release to load the file", "Solte para carregar o arquivo"),
    (
        "Drop a waveform file here (FNIRSI .wav, Keysight .bin, Rigol .bin/.wfm, Digilent WaveForms, VCD, or CSV), or use File ▾ → Open file…",
        "Solte aqui um arquivo de forma de onda (FNIRSI .wav, Keysight .bin, Rigol .bin/.wfm, Digilent WaveForms, VCD ou CSV), ou use Arquivo ▾ → Abrir arquivo…",
    ),
    ("Logic", "Lógico"),
    ("Divisions", "Divisões"),
    ("Voltage", "Tensão"),
    ("Time (trigger at t = 0)", "Tempo (gatilho em t = 0)"),
    (" {a0} dBV", " {a0} dBV"),
    (
        "⚠ zoom in: the span shown is sampled every {a0}, the shortest pulse is {a1}",
        "⚠ dê zoom: o intervalo mostrado é amostrado a cada {a0}, o pulso mais curto tem {a1}",
    ),
    (
        "The time span shown has {total} samples; the FFT uses the first {count} (from the left edge). Zoom in to analyse another part.",
        "O intervalo de tempo mostrado tem {total} amostras; a FFT usa as primeiras {count} (a partir da borda esquerda). Dê zoom para analisar outra parte.",
    ),
    (
        "CH{a0}: 0 V. Drag to move the channel up or down.",
        "CH{a0}: 0 V. Arraste para subir ou descer o canal.",
    ),
    (
        "CH{a0}: 0 V. Drag to move the channel up or down (switches Y to Div: in Volts the channels share one axis).",
        "CH{a0}: 0 V. Arraste para subir ou descer o canal (muda Y para Div: em Volts os canais compartilham um eixo).",
    ),
    (
        "Bus of digital signals. Click to hide or show it; right click (or long press): values in Hex, Dec, Bin or ASCII.",
        "Barramento de sinais digitais. Clique para esconder ou mostrar; botão direito (ou toque longo): valores em Hex, Dec, Bin ou ASCII.",
    ),
    ("Bus values:", "Valores do barramento:"),
    ("logic", "lógico"),
    (
        "Digital signal (0/1). Click to hide or show it.",
        "Sinal digital (0/1). Clique para esconder ou mostrar.",
    ),
    (
        "Search menu: [ and ] go to the previous and next event",
        "Menu Buscar: [ e ] vão para o evento anterior e o próximo",
    ),
    (
        "Reference waveform (File ▾ → Open reference…). Click to hide or show it.",
        "Forma de onda de referência (Arquivo ▾ → Abrir referência…). Clique para esconder ou mostrar.",
    ),
    (
        "The signal reaches the limit of the input range: amplitude measurements are marked with \"?\"",
        "O sinal chega ao limite da faixa de entrada: as medições de amplitude aparecem com \"?\"",
    ),
    ("Record", "Registro"),
    ("Screen", "Tela"),
    (
        "The samples measured: Measure ▾ → Samples",
        "As amostras medidas: Medir ▾ → Amostras",
    ),
    (
        "Click to hide this channel",
        "Clique para esconder este canal",
    ),
    (
        "Click to show this channel",
        "Clique para mostrar este canal",
    ),
    ("{bits}-bit bus · {a0}", "barramento de {bits} bits · {a0}"),
    (
        "Mouse wheel: {unit}/div of {name} (1-2-5 steps; switches Y to Div). In the file: {a0}/div.",
        "Roda do mouse: {unit}/div de {name} (passos 1-2-5; muda Y para Div). No arquivo: {a0}/div.",
    ),
    ("ETS · frame 1 of {frames}", "ETS · quadro 1 de {frames}"),
    ("⚠ {a0} clipped", "⚠ {a0} saturado"),
    ("Measurements · {window_name}", "Medições · {window_name}"),
    (
        "{a0}\n⚠ {name} is clipped in this window: the real value can be larger",
        "{a0}\n⚠ {name} está saturado nesta janela: o valor real pode ser maior",
    ),
    ("{name} per cycle", "{name} por ciclo"),
    ("Freq", "Freq"),
    ("Period", "Período"),
    ("Vpp", "Vpp"),
    ("Vmax", "Vmax"),
    ("Vmin", "Vmin"),
    ("Vtop", "Vtop"),
    ("Vbase", "Vbase"),
    ("Vamp", "Vamp"),
    ("Mean", "Média"),
    ("RMS", "RMS"),
    ("Rise", "Subida"),
    ("Fall", "Descida"),
    ("+Width", "+Largura"),
    ("−Width", "−Largura"),
    ("+Duty", "+Ciclo"),
    ("Frequency: 1 / period", "Frequência: 1 / período"),
    (
        "Period: mean time between rising edges",
        "Período: tempo médio entre bordas de subida",
    ),
    (
        "Peak-to-peak voltage: Vmax − Vmin",
        "Tensão pico a pico: Vmax − Vmin",
    ),
    ("Highest sample", "Maior amostra"),
    ("Lowest sample", "Menor amostra"),
    (
        "Top level (most common high level)",
        "Nível alto (o nível alto mais comum)",
    ),
    (
        "Base level (most common low level)",
        "Nível baixo (o nível baixo mais comum)",
    ),
    ("Amplitude: Vtop − Vbase", "Amplitude: Vtop − Vbase"),
    ("Mean of the samples", "Média das amostras"),
    ("RMS of the samples (AC + DC)", "RMS das amostras (AC + DC)"),
    ("Rise time, 10% to 90%", "Tempo de subida, de 10% a 90%"),
    ("Fall time, 90% to 10%", "Tempo de descida, de 90% a 10%"),
    (
        "Positive pulse width, at 50%",
        "Largura do pulso positivo, em 50%",
    ),
    (
        "Negative pulse width, at 50%",
        "Largura do pulso negativo, em 50%",
    ),
    (
        "Positive duty cycle: +Width / period",
        "Ciclo de trabalho positivo: +Largura / período",
    ),
    ("T1–T2", "T1–T2"),
    ("Rising edges", "Bordas de subida"),
    ("Falling edges", "Bordas de descida"),
    ("Glitches", "Glitches"),
    ("Rectangular", "Retangular"),
    ("Hanning", "Hanning"),
    ("Hamming", "Hamming"),
    ("Blackman-Harris", "Blackman-Harris"),
    ("Flat top", "Flat top"),
    (
        "No window: transients, or a whole number of periods",
        "Sem janela: transitórios, ou um número inteiro de períodos",
    ),
    (
        "General use with periodic signals",
        "Uso geral com sinais periódicos",
    ),
    (
        "Like Hanning, slightly narrower peak",
        "Como a Hanning, pico um pouco mais estreito",
    ),
    (
        "Low leakage: small harmonics next to a large peak",
        "Pouco vazamento: harmônicas pequenas ao lado de um pico grande",
    ),
    (
        "Accurate amplitude, wide peak",
        "Amplitude precisa, pico largo",
    ),
    ("parity error", "erro de paridade"),
    (
        "framing error (stop bit is 0)",
        "erro de enquadramento (bit de parada em 0)",
    ),
    (
        "incomplete byte (cut by a start or stop)",
        "byte incompleto (cortado por um start ou stop)",
    ),
    ("UART", "UART"),
    ("I2C", "I2C"),
    ("SPI", "SPI"),
    ("Even", "Par"),
    ("Odd", "Ímpar"),
    ("mean", "média"),
    ("min", "mín."),
    ("max", "máx."),
    ("σ", "σ"),
    ("n", "n"),
    // Decoders: titles and warnings.
    (" (auto)", " (auto)"),
    (" inverted", " invertido"),
    (", no CS", ", sem CS"),
    (
        "SCLK {a0}, mode {mode}, {a1}, {word_bits} bits",
        "SCLK {a0}, modo {mode}, {a1}, {word_bits} bits",
    ),
    (
        "choose the clock line ({a0}) in the ▸ settings",
        "escolha a linha de clock ({a0}) nos ajustes ▸",
    ),
    (
        "measured {b} baud, not a standard rate: set the baud rate if the bytes are wrong",
        "medidos {b} baud, não é uma taxa padrão: ajuste o baud rate se os bytes estiverem errados",
    ),
    ("no clock line", "sem linha de clock"),
    (
        "no clock: SCL does not change",
        "sem clock: o SCL não varia",
    ),
    (
        "no clock: SCLK does not change",
        "sem clock: o SCLK não varia",
    ),
    (
        "no edges: the signal does not change",
        "sem bordas: o sinal não varia",
    ),
    (
        "only {a0} samples per bit: the bytes may be wrong (use a faster time base)",
        "só {a0} amostras por bit: os bytes podem estar errados (use uma base de tempo mais rápida)",
    ),
    (
        "only {a0} samples per clock period: the bytes may be wrong (use a faster time base)",
        "só {a0} amostras por período de clock: os bytes podem estar errados (use uma base de tempo mais rápida)",
    ),
    (
        "only {a0} samples per clock period: the words may be wrong (use a faster time base)",
        "só {a0} amostras por período de clock: as palavras podem estar erradas (use uma base de tempo mais rápida)",
    ),
    ("{a0} MHz", "{a0} MHz"),
    ("{a0} kHz", "{a0} kHz"),
    ("{f} Hz", "{f} Hz"),
    ("{words} words", "{words} palavras"),
    // Used by the i18n tests.
    ("{n} words", "{n} palavras"),
];
