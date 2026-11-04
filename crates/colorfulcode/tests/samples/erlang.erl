%% A tiny key value server built on gen_server.
-module(kv_server).
-behaviour(gen_server).

-export([start_link/0, put/2, get/1]).
-export([init/1, handle_call/3, handle_cast/2]).

-define(SERVER, ?MODULE).
-define(TIMEOUT, 5000).

-record(state, {store = #{} :: map(), hits = 0 :: non_neg_integer()}).

-spec start_link() -> {ok, pid()} | {error, term()}.
start_link() ->
    gen_server:start_link({local, ?SERVER}, ?MODULE, [], []).

put(Key, Value) ->
    gen_server:cast(?SERVER, {put, Key, Value}).

get(Key) ->
    gen_server:call(?SERVER, {get, Key}, ?TIMEOUT).

init([]) ->
    io:format("starting ~p~n", [?MODULE]),
    {ok, #state{}}.

handle_call({get, Key}, _From, #state{store = Store, hits = Hits} = State) ->
    Reply = case maps:find(Key, Store) of
        {ok, Value} -> {ok, Value};
        error -> {error, not_found}
    end,
    {reply, Reply, State#state{hits = Hits + 1}};
handle_call(_Request, _From, State) ->
    {reply, ignored, State}.

handle_cast({put, Key, Value}, State = #state{store = Store}) ->
    {noreply, State#state{store = Store#{Key => Value}}}.

checksum(Bin) when is_binary(Bin) ->
    lists:foldl(fun(B, Acc) -> (Acc + B) band 16#FF end, 0, binary_to_list(Bin)).

describe(N) when N > 100, N rem 2 =:= 0 -> 'big even';
describe(N) when N > 100 -> "big\todd\n";
describe(_) ->
    Chars = [$a, $\n, $\s],
    [X * 2 || X <- [1, 2, 3.5e1], X =/= 0] ++ Chars.
