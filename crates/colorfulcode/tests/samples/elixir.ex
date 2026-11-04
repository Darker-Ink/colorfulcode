defmodule MyApp.Accounts.User do
  @moduledoc """
  User accounts and #{String.upcase("helpers")}.
  """
  use Ecto.Schema
  import Ecto.Changeset

  @max_age 150
  @type t :: %__MODULE__{name: String.t(), age: integer}

  schema "users" do
    field :name, :string
    field :age, :integer, default: 0
  end

  @doc "Builds a changeset."
  def changeset(user, attrs \\ %{}) do
    user
    |> cast(attrs, [:name, :age])
    |> validate_required([:name])
    |> validate_number(:age, less_than: @max_age)
  end

  defp adult?(%{age: age}) when age >= 18, do: true
  defp adult?(_), do: false

  def greet(name) do
    case Regex.run(~r/^(\w+)\s/i, name) do
      [_, first] -> "Hello, #{first}!\n"
      nil -> :"no match"
    end
  end

  def words, do: ~w(alpha beta gamma)a
  def handle(list), do: Enum.map(list, &(&1 * 0x10))

  def spawn_worker do
    pid = spawn(fn -> receive do {:ping, from} -> send(from, :pong) end end)
    {:ok, pid, 'charlist', 1_000.5}
  end
end
