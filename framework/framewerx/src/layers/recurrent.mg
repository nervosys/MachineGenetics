// framewerx::layers::recurrent — RNN / LSTM / GRU cells and stacks
//
// The RMI low-level layer doesn't yet have a dedicated RNN opcode -
// recurrent computation is expressed as a Control::Loop over a Linear
// cell. The wrappers here let agents declare the cells declaratively;
// the bridge lowers them to the appropriate composition.

// Basic Elman RNN cell. h_t = tanh(W_ih * x_t + b_ih + W_hh * h_{t-1} + b_hh)
S RNNCell {
    input_size: usize,
    hidden_size: usize,
}

I RNNCell {
    +f new(input_size: usize, hidden_size: usize) -> RNNCell {
        @RNNCell { input_size: input_size, hidden_size: hidden_size }
    }
}

// LSTM cell: forget / input / candidate / output gates.
S LSTMCell {
    input_size: usize,
    hidden_size: usize,
}

I LSTMCell {
    +f new(input_size: usize, hidden_size: usize) -> LSTMCell {
        @LSTMCell { input_size: input_size, hidden_size: hidden_size }
    }
}

// GRU cell: reset + update gates, simpler than LSTM, often matches it.
S GRUCell {
    input_size: usize,
    hidden_size: usize,
}

I GRUCell {
    +f new(input_size: usize, hidden_size: usize) -> GRUCell {
        @GRUCell { input_size: input_size, hidden_size: hidden_size }
    }
}

// Multi-layer wrappers: stack a recurrent cell N times with optional
// dropout between layers and bidirectional support.
S RNN {
    input_size: usize,
    hidden_size: usize,
    num_layers: usize,
    bidirectional: bool,
    dropout: f32,
}

I RNN {
    +f new(input_size: usize, hidden_size: usize, num_layers: usize) -> RNN {
        @RNN {
            input_size: input_size,
            hidden_size: hidden_size,
            num_layers: num_layers,
            bidirectional: 0b,
            dropout: 0.0,
        }
    }
}

S LSTM {
    input_size: usize,
    hidden_size: usize,
    num_layers: usize,
    bidirectional: bool,
    dropout: f32,
}

I LSTM {
    +f new(input_size: usize, hidden_size: usize, num_layers: usize) -> LSTM {
        @LSTM {
            input_size: input_size,
            hidden_size: hidden_size,
            num_layers: num_layers,
            bidirectional: 0b,
            dropout: 0.0,
        }
    }

    +f bidir(input_size: usize, hidden_size: usize, num_layers: usize) -> LSTM {
        @LSTM {
            input_size: input_size,
            hidden_size: hidden_size,
            num_layers: num_layers,
            bidirectional: 1b,
            dropout: 0.0,
        }
    }
}

S GRU {
    input_size: usize,
    hidden_size: usize,
    num_layers: usize,
    bidirectional: bool,
    dropout: f32,
}

I GRU {
    +f new(input_size: usize, hidden_size: usize, num_layers: usize) -> GRU {
        @GRU {
            input_size: input_size,
            hidden_size: hidden_size,
            num_layers: num_layers,
            bidirectional: 0b,
            dropout: 0.0,
        }
    }
}
