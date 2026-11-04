' Number guessing game in QBasic
DECLARE SUB ShowBanner (title$)
DECLARE FUNCTION Clamp% (v%, lo%, hi%)

CONST MAXTRIES = 7
DIM SHARED score AS INTEGER
DIM names$(10)

RANDOMIZE TIMER
CLS
ShowBanner "Guess the number"
secret% = INT(RND * 100) + 1
tries% = 0

DO
    INPUT "Your guess"; guess%
    guess% = Clamp%(guess%, 1, 100)
    tries% = tries% + 1
    IF guess% < secret% THEN
        PRINT "Too low!"
    ELSEIF guess% > secret% THEN
        PRINT "Too high!"
    ELSE
        PRINT "Correct in"; tries%; "tries"
        EXIT DO
    END IF
LOOP UNTIL tries% >= MAXTRIES

REM print a summary
PRINT USING "Score: ###.##"; score / 3.5
PRINT LEFT$("Goodbye", 4) + CHR$(33); &HFF
END

10 GOTO 20
20 GOSUB 30

SUB ShowBanner (title$)
    COLOR 14, 1
    PRINT STRING$(LEN(title$), "*")
    PRINT UCASE$(title$)
END SUB

FUNCTION Clamp% (v%, lo%, hi%)
    IF v% < lo% THEN v% = lo%
    IF v% > hi% THEN v% = hi%
    Clamp% = v%
END FUNCTION
