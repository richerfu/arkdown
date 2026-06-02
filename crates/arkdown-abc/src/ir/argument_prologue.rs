use crate::emit::strings::StringTable;
use crate::ir::opcodes::Insn;

pub(super) fn prepend_function_argument_copies(
    mut bytecode: Vec<u8>,
    num_vregs: u16,
    num_args: u16,
    mandatory_this_reg: u8,
) -> Vec<u8> {
    if num_args == 0 {
        return bytecode;
    }

    let has_this_lexenv = has_initial_lexenv(&bytecode);
    let mut prologue = argument_copy_prologue(
        num_vregs,
        num_args,
        has_this_lexenv.then_some(mandatory_this_reg),
    );

    if has_this_lexenv {
        let lexenv: Vec<u8> = bytecode.drain(..4).collect();
        let this_src = num_vregs + mandatory_this_reg as u16;
        let mut with_lexenv = lexenv;
        with_lexenv.append(&mut prologue);
        if this_src <= u8::MAX as u16 {
            with_lexenv.extend_from_slice(&encode(Insn::Lda(this_src as u8)));
            with_lexenv.extend_from_slice(&encode(Insn::StLexVar(0, 0)));
        }
        with_lexenv.append(&mut bytecode);
        return with_lexenv;
    }

    prologue.append(&mut bytecode);
    prologue
}

pub(super) fn prepend_module_argument_copies(
    mut bytecode: Vec<u8>,
    num_vregs: u16,
    num_args: u16,
) -> Vec<u8> {
    if num_args == 0 {
        return bytecode;
    }
    let mut prologue = argument_copy_prologue(num_vregs, num_args, None);

    if has_initial_lexenv(&bytecode) {
        let mut with_lexenv: Vec<u8> = bytecode.drain(..4).collect();
        with_lexenv.append(&mut prologue);
        with_lexenv.append(&mut bytecode);
        return with_lexenv;
    }

    prologue.append(&mut bytecode);
    prologue
}

fn argument_copy_prologue(num_vregs: u16, num_args: u16, skip_dst: Option<u8>) -> Vec<u8> {
    let mut prologue = Vec::new();
    for dst in 0..num_args {
        if skip_dst.is_some_and(|skip_dst| dst == skip_dst as u16) {
            continue;
        }
        let src = num_vregs + dst;
        prologue.extend_from_slice(&encode_mov(dst, src));
    }
    prologue
}

fn encode_mov(dst: u16, src: u16) -> Vec<u8> {
    if dst <= 0x0f && src <= 0x0f {
        return encode(Insn::MovV4(dst as u8, src as u8));
    }
    if src <= u8::MAX as u16 {
        return encode(Insn::MovV8(dst as u8, src as u8));
    }
    encode(Insn::MovV16(dst, src))
}

fn has_initial_lexenv(bytecode: &[u8]) -> bool {
    bytecode.first() == Some(&0xb6) && bytecode.get(1) == Some(&1)
}

fn encode(insn: Insn) -> Vec<u8> {
    insn.encode(&mut StringTable::default())
}
