# TSFFS Coverage Feedback

Terms for the execution feedback used to guide fuzzing.

## Language

**Branch coverage**:
Feedback based on the destinations reached by executed branches, including calls and returns. Repeated executions contribute to coverage even when the instructions have executed before.
_Avoid_: Once coverage

**Comparison logging**:
Recording comparison operand values observed during execution to guide value-driven mutations.
_Avoid_: Compare coverage

**Comparison-logging execution**:
A testcase execution selected to collect comparison operands. Enabling comparison logging for a fuzzing campaign does not make every testcase execution a comparison-logging execution.

**Selective instrumentation**:
Observation of branch and comparison instructions for fuzzing feedback, without observing every instruction.
_Avoid_: Once mode

**Full-instruction instrumentation**:
Observation of every executed instruction, needed for complete execution traces and symbolic or source coverage as well as fuzzing feedback.
_Avoid_: Legacy mode
