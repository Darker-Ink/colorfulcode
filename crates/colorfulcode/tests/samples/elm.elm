module Main exposing (Model, Msg(..), main, update)

{-| A counter with a history of changes.
{- nested -}
-}

import Browser
import Html exposing (Html, button, div, text)
import Html.Events exposing (onClick)


type alias Model =
    { count : Int
    , history : List Int
    }


type Msg
    = Increment
    | Decrement
    | Reset


init : Model
init =
    { count = 0, history = [] }


update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1, history = model.count :: model.history }

        Decrement ->
            { model | count = model.count - 1 }

        Reset ->
            init


view : Model -> Html Msg
view model =
    div []
        [ button [ onClick Decrement ] [ text "-" ]
        , text (String.fromInt model.count ++ " clicks\n")
        , button [ onClick Increment ] [ text "+" ]
        , text (String.fromChar 'x' ++ """multi
line""")
        ]


main : Program () Model Msg
main =
    Browser.sandbox { init = init, update = update, view = view }
