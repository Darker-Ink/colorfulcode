package com.example.game {
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.utils.getTimer;

    /**
     * Bounces a ball around the stage.
     */
    [SWF(width="800", height="600", frameRate="60")]
    public class Bouncer extends Sprite {
        public static const GRAVITY:Number = 0.5;
        private var ball:Sprite;
        private var vx:Number = 4;
        private var vy:Number = 0;
        private var bounces:int = 0;

        [Bindable]
        public var label:String = "ready";

        public function Bouncer() {
            ball = new Sprite();
            ball.graphics.beginFill(0xFF3300);
            ball.graphics.drawCircle(0, 0, 12.5);
            addChild(ball);
            addEventListener(Event.ENTER_FRAME, onFrame);
        }

        public function get speed():Number {
            return Math.sqrt(vx * vx + vy * vy);
        }

        private function onFrame(e:Event):void {
            vy += GRAVITY;
            ball.x += vx;
            ball.y += vy;
            if (ball.y > stage.stageHeight) {
                ball.y = stage.stageHeight;
                vy *= -0.8;
                bounces++;
                label = 'bounces: ' + bounces + "\n";
            }
            var pattern:RegExp = /ball_(\d+)/g;
            trace(pattern.test(name), getTimer() / 1e3, this.speed);
        }
    }
}
