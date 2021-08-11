/// Interrupt operations.
pub enum Interrupt {
    /** 
        Vector $FFFA/$FFFB | Push PC and P

        Same as BRK but $FFFA/$FFFB are put into the address bus.

        7 cycles
    */
    NMI,
    /**
        Vector $FFFC/$FFFD

        #1 SP = $00 READ $00FF

        #2 SP = $00 READ $00FF

        #3 SP = $00 READ $01FF - Dec SP

        #4 SP = $FF READ $01FF - Dec SP

        #5 SP = $FE READ $01FE - Dec SP

        #6 SP = $FD READ $FFFC - Low Byte

        #7 SP = $FD READ $FFFD - High Byte

        #8 SP = $FD AB = $HILO from previous reads.

        8 cycles
    */
    RESET,
    /**
        Vector $FFFD/$FFFF | Push PC and P

        Same as BRK, but clears the B flag.

        6 cycles
    */
    IRQ,
    /**
        Vector $FFFD/$FFFF | Push PC and P | Set B Flag

        #1 Store PC(hi)

        #2 Store PC(lo)

        #3 Store P
        
        #4 Fetch PC(lo) from $FFFE
        
        #5 Fetch PC(hi) from $FFFF

        #6 Call instruction

        6 cycles.
    */
    BRK,
}