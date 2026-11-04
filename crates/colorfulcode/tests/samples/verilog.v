// Simple UART transmitter
`timescale 1ns / 1ps
`define CLKS_PER_BIT 87

module uart_tx #(
    parameter DATA_BITS = 8
) (
    input  wire       clk,
    input  wire       rst_n,
    input  wire [7:0] data,
    input  wire       start,
    output reg        tx,
    output reg        busy
);

    localparam IDLE = 2'b00, START = 2'b01, DATA = 2'b10, STOP = 2'b11;

    reg [1:0]  state;
    reg [15:0] clk_count;
    reg [2:0]  bit_index;
    integer    i;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= IDLE;
            tx    <= 1'b1;
            busy  <= 1'b0;
        end else begin
            case (state)
                IDLE: if (start) begin
                    state     <= START;
                    busy      <= 1'b1;
                    clk_count <= 16'h0000;
                end
                START: begin
                    tx <= 1'b0;
                    if (clk_count == `CLKS_PER_BIT - 1) state <= DATA;
                    else clk_count <= clk_count + 1;
                end
                default: state <= IDLE;
            endcase
        end
    end

    initial $display("uart_tx ready, width=%0d\n", DATA_BITS);
endmodule

/* instantiation */
uart_tx #(.DATA_BITS(8)) u_tx (.clk(clk), .rst_n(rst_n), .data(8'hA5));
