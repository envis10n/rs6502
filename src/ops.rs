/// Mode used to obtain the proper address for an operand during an instruction.
pub enum AddressingMode {
    /// No operand provided (e.g. INX)
    Implied,
    /// Accumulator operand A 1 byte instruction (e.g. ROL)
    Accumulator,
    /// Operand is 1 byte and is at the current PC (e.g. LDA #$05 -> Load $05 to the accumulator)
    Immediate,
    /**
        Operand is the LSB of the EA, MSB is $00.
        
        $0000 - $00FF (LDA $56 -> EA $0056 => $1D : Load from $0056 to the accumulator)
    */
    ZeroPage,
    /** 
        Operand is added with no carry to the index register X.
        
        The result is the LSB of the EA. MSB is $00.
    */
    ZeroPageX,
    /** 
        Operand is added with no carry to the index register Y.
        
        The result is the LSB of the EA. MSB is $00.
    */
    ZeroPageY,
    /// Operand is the 2 byte effective address.
    Absolute,
    /// Operand is added to the index register X. The result is the EA.
    AbsoluteX,
    /// Operand is added to the index register Y. The result is the EA.
    AbsoluteY,
    /** 
        The operand contains the LSB of the EA.

        The address $aaaa + $0001 contains the MSB of the EA.

        2 byte address (($aaaa + $0001) * $100) + $aaaa contains the EA.
    */
    Indirect,
    /**
        The operand $aa is added with no carry to the index register.
        
        The resulting zero page address $aa + X contains the LSB of the EA.
        
        The address $aa + X + $01 contains the MSB of the EA.
    */
    IndirectX,
    /**
        The operand $aa is added with no carry to the index register.
        
        The resulting zero page address $aa + Y contains the LSB of the EA.
        
        The address $aa + Y + $01 contains the MSB of the EA.
    */
    IndirectY,
    /// The operand $aa is added to the LSB of the PC as an offset.
    Relative,
}