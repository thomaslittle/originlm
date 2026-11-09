from http.server import HTTPServer, BaseHTTPRequestHandler
from PIL import Image, ImageDraw
import io,base64,json,pathlib
root=pathlib.Path(__file__).parent
im=Image.new('RGB',(1920,1080),'#132338');d=ImageDraw.Draw(im)
d.rectangle((120,460,1800,620),fill='#294865');d.text((160,510),'MOCK PROVIDER - STARTING SOON - UI TEST',fill='white',stroke_width=1)
b=io.BytesIO();im.save(b,format='PNG');im.save(root/'mock-provider.png')
class Handler(BaseHTTPRequestHandler):
 def do_POST(self):
  data=self.rfile.read(int(self.headers.get('Content-Length','0')))
  (root/'request.json').write_bytes(data)
  out=json.dumps({'data':[{'b64_json':base64.b64encode(b.getvalue()).decode()}]}).encode()
  self.send_response(200);self.send_header('Content-Type','application/json');self.end_headers();self.wfile.write(out)
 def log_message(self,*args):pass
HTTPServer(('127.0.0.1',18763),Handler).serve_forever()
