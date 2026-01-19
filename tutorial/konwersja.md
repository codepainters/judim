# Konwersja gry Jet Set Willy na Juniora

## Etap 1: ekstrakcja

Zaczynamy od pliku [jetset.tap](../tests/jetset.tap) - sprawdzamy jego zawartość:

```sh 
$ judim tap jetset.tap info
0: "Jetset1"
    type: BASIC Program
    size: 336
    autostart: 10
    vars offet: 317

1: "Jetset2"
    type: Code/bytes
    size: 32768
    load address: 0x8000
```

Jak widać mamy tu tylko dwa pliki - loader w Basicu i kod maszynowy właściwej gry.

Możemy je wyekstrahować na poleceniem `extract` lub `explode`, użyjmy tutaj tego pierwszego. Polecenie to ma następujące opcje:

```sh
$ judim tap jetset.tap extract -h
Extract individual file from the .tap file

Usage: judim tap <TAP_FILE> extract [OPTIONS] --index <INDEX> <OUTPUT_FILE>

Arguments:
  <OUTPUT_FILE>  Output file name

Options:
  -i, --index <INDEX>  Index of the file to extract
      --header         Extract only the raw header bytes
  -d, --data           Extract only the raw data bytes
  -n, --no-autorun     Disable autorun (Basic only)
  -h, --help           Print help
```

W przypadku programu w Basicu opcją `-n` blokujemy autorun - nie chcemy, aby loader startował automatycznie, chcemy go móc bez kłopotu zmodyfikować. UWAGA: rozszerzenie są istotne - polecenie `LOAD` bierze tylko nazwę, dokładając rozszerzenie w zależności od typu ładowanego pliku (`.PRG` dla programu w Basicu, `.COD` dla kodu maszynowego).   Do dzieła:

```sh
$ judim tap jetset.tap extract -i 0 -n jetset.prg 
$ judim tap jetset.tap extract -i 1 jetset.cod
$ ls -l jetset.*
-rw-rw-r-- 1 czajnik czajnik 32785 sty 19 22:26 jetset.cod
-rw-rw-r-- 1 czajnik czajnik   353 sty 19 22:26 jetset.prg
-rw-rw-r-- 1 czajnik czajnik 33154 sty 19 22:19 jetset.tap
```

Pliki możemy teraz przekopiować na dyskietkę (lub obraz dyskietki) dowolnym sposobem.

**TODO** - opisać, gdy Judim będzie to robił.

## Etap 2: modyfikacja loadera

Teraz możemy załadować loader na Juniorze i wylistować jego zawartość:

```basic
LOAD *"jetset"
LIST
```

Powinniśmy zobaczyć taki kod:

```basic
10 CLEAR 25000: PAPER 1: BORDER 1: CLS : FOR a=0 TO 12 STEP 2: BEEP .1,a: NEXT a: PAPER 6: INK 0: PRINT AT 10,5;"                      ";AT 11,5;" JetSet Willy Loading ";AT 12,5;"                      "
11 INK 1: PAPER 1: POKE 23613,0
30 LOAD "Jetset2"CODE 
40 RANDOMIZE USR 33792
```

Chcemy aby loader załadował właściwą grę z pliku `JETSET.COD` z dyskietki. Cała nasza konwersja sprowadza się więc do zmiany linii 30 na:

```basic
30 LOAD *"jetset" CODE
```

Zapisujemy zmodyfikowany program (tym razem ustawiając ponownie auto-start od linii 10):

```basic
SAVE *"jetset" LINE 10
```

I gotowe!

## Etap 3: testowanie

Aby uruchomić grę z dyskietki, wystarczy polecenie:

```basic
LOAD *"jetest"
```

Może przydać się [ta tabela](https://skoolkit.ca/disassemblies/jet_set_willy/tables/codes.html) :).

