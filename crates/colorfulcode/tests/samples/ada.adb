--  Bounded stack package with a small test driver
with Ada.Text_IO; use Ada.Text_IO;

procedure Stack_Demo is

   Max_Size : constant := 16#FF#;

   type Index is range 0 .. 100;
   subtype Small is Integer range 1 .. 10;
   type Int_Array is array (Index range <>) of Integer;

   package Stacks is
      type Stack is tagged private;
      procedure Push (S : in out Stack; Item : Integer);
      function Pop (S : in out Stack) return Integer;
      Underflow : exception;
   private
      type Stack is tagged record
         Data : Int_Array (1 .. 100) := (others => 0);
         Top  : Index := 0;
      end record;
   end Stacks;

   package body Stacks is
      procedure Push (S : in out Stack; Item : Integer) is
      begin
         S.Top := S.Top + 1;
         S.Data (S.Top) := Item;
      end Push;

      function Pop (S : in out Stack) return Integer is
      begin
         if S.Top = 0 then
            raise Underflow with "stack is empty";
         end if;
         S.Top := S.Top - 1;
         return S.Data (S.Top + 1);
      end Pop;
   end Stacks;

   S : Stacks.Stack;
   C : Character := 'A';
   F : Float := 1.5E-3;
begin
   for I in Small'First .. Small'Last loop
      S.Push (I * 1_000);
   end loop;
   Put_Line ("Popped:" & Integer'Image (S.Pop) & " char " & C);
   pragma Assert (F > 0.0 and then Max_Size = 255);
exception
   when Stacks.Underflow => Put_Line ("Oops, ""empty""");
end Stack_Demo;
