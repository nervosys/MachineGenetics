// LSTM language model. Embedding -> 2-layer LSTM -> linear head over vocab.

net LSTMLanguageModel {
    layer embed: Embed(50000, 256);
    layer rnn1: Linear(256, 1024);
    layer act: ReLU;
    layer rnn2: Linear(1024, 256);
    layer head: Linear(256, 50000);
    forward { head(rnn2(act(rnn1(embed)))) }
}
