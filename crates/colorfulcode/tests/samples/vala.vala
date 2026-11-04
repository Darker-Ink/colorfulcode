using GLib;

/* A tiny note taking app. */
namespace Notes {
    public errordomain NoteError {
        EMPTY,
        TOO_LONG
    }

    [CCode (cname = "notes_note")]
    public class Note : Object {
        public const int MAX_LENGTH = 280;

        public string title { get; set; default = "untitled"; }
        public string body { get; construct; }
        public DateTime created { get; private set; }

        public signal void changed (string field);

        public Note (string title, string body) {
            Object (body: body);
            this.title = title;
            this.created = new DateTime.now_local ();
        }

        public void validate () throws NoteError {
            if (body.length == 0) {
                throw new NoteError.EMPTY ("note '%s' is empty", title);
            }
            if (body.length > MAX_LENGTH) {
                throw new NoteError.TOO_LONG (@"note $title has $(body.length) chars");
            }
        }
    }

    public static int main (string[] args) {
        var note = new Note ("Groceries", """milk
eggs""");
        note.changed.connect ((field) => {
            print ("changed: %s\n", field);
        });
        try {
            note.validate ();
        } catch (NoteError e) {
            stderr.printf ("error: %s\n", e.message);
            return 1;
        }
        char c = 'x';
        return 0x0 + (int) c - 120;
    }
}
