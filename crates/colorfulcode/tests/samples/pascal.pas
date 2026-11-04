{ Simple stack implementation in Object Pascal }
program StackDemo;

{$mode objfpc}{$H+}

uses
  SysUtils, Classes;

type
  TIntStack = class(TObject)
  private
    FItems: array of Integer;
    FCount: Integer;
  public
    constructor Create;
    procedure Push(Value: Integer);
    function Pop: Integer;
    property Count: Integer read FCount;
  end;

  EStackEmpty = class(Exception);

constructor TIntStack.Create;
begin
  inherited Create;
  FCount := 0;
end;

procedure TIntStack.Push(Value: Integer);
begin
  if FCount >= Length(FItems) then
    SetLength(FItems, FCount * 2 + 4);
  FItems[FCount] := Value;
  Inc(FCount);
end;

function TIntStack.Pop: Integer;
begin
  if FCount = 0 then
    raise EStackEmpty.Create('Stack is empty');
  Dec(FCount);
  Result := FItems[FCount];
end;

var
  S: TIntStack;
  I: Integer;
begin
  S := TIntStack.Create;
  try
    for I := 1 to 10 do
      S.Push(I * $FF + %101);  // hex and binary
    (* print the top *)
    WriteLn('Top: ', S.Pop, #13#10, 'It''s done', 3.14e2);
  finally
    FreeAndNil(S);
  end;
end.
